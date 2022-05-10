use std::{num::NonZeroU32, sync::Arc};

use bb8_redis::{bb8::Pool, RedisConnectionManager};
use dashmap::{DashMap, DashSet};
use flurry::HashMap as FlurryMap;
use hashbrown::{HashMap, HashSet};
use parking_lot::Mutex;
use rosu_v2::Osu;
use smallvec::SmallVec;
use tokio::sync::{mpsc::UnboundedSender, Mutex as TokioMutex, RwLock};
use twilight_gateway::{cluster::Events, Cluster};
use twilight_http::{client::InteractionClient, Client};
use twilight_model::{
    channel::message::allowed_mentions::AllowedMentionsBuilder,
    id::{
        marker::{ApplicationMarker, ChannelMarker, GuildMarker, MessageMarker, UserMarker},
        Id,
    },
};
use twilight_standby::Standby;

use crate::{
    core::CONFIG,
    custom_client::CustomClient,
    database::{Database, GuildConfig},
    games::{
        bg::GameState as BgGameState,
        hl::{retry::RetryState, GameState as HlGameState},
    },
    matchlive::MatchLiveChannels,
    server::AuthenticationStandby,
    tracking::OsuTracking,
    util::CountryCode,
    BotResult,
};

use super::{buckets::Buckets, cluster::build_cluster, BotStats, Cache, RedisCache};

mod background_loop;
mod configs;
mod countries;
mod games;
mod map_collect;
mod matchlive;
mod messages;
mod role_assign;
mod shutdown;
mod twitch;

pub type Redis = Pool<RedisConnectionManager>;
pub type AssignRoles = SmallVec<[u64; 1]>;

pub struct Context {
    pub auth_standby: AuthenticationStandby,
    pub buckets: Buckets,
    pub cache: Cache,
    pub cluster: Cluster,
    pub http: Arc<Client>,
    pub member_requests: MemberRequests,
    pub standby: Standby,
    pub stats: Arc<BotStats>,
    // private to avoid deadlocks by messing up references
    data: ContextData,
    clients: Clients,
}

impl Context {
    pub fn interaction(&self) -> InteractionClient<'_> {
        self.http.interaction(self.data.application_id)
    }

    pub fn osu(&self) -> &Osu {
        &self.clients.osu
    }

    pub fn psql(&self) -> &Database {
        &self.clients.psql
    }

    /// Returns the custom client
    pub fn client(&self) -> &CustomClient {
        &self.clients.custom
    }

    /// Return the plain redis connection pool
    pub fn redis_client(&self) -> &Redis {
        &self.clients.redis
    }

    /// Return a redis wrapper with a specific interface
    pub fn redis(&self) -> RedisCache<'_> {
        RedisCache::new(self)
    }

    pub fn tracking(&self) -> &OsuTracking {
        &self.data.osu_tracking
    }

    pub async fn new(tx: UnboundedSender<(Id<GuildMarker>, u64)>) -> BotResult<(Self, Events)> {
        let config = CONFIG.get().unwrap();
        let discord_token = &config.tokens.discord;

        let mentions = AllowedMentionsBuilder::new()
            .replied_user()
            .roles()
            .users()
            .build();

        // Connect to the discord http client
        let http = Client::builder()
            .token(discord_token.to_owned())
            .remember_invalid_token(false)
            .default_allowed_mentions(mentions)
            .build();

        let http = Arc::new(http);

        let current_user = http.current_user().exec().await?.model().await?;
        let application_id = current_user.id.cast();

        info!(
            "Connecting to Discord as {}#{}...",
            current_user.name, current_user.discriminator
        );

        // Connect to psql database
        let psql = Database::new(&config.database_url)?;

        // Connect to redis
        let redis_host = &config.redis_host;
        let redis_port = config.redis_port;
        let redis_uri = format!("redis://{redis_host}:{redis_port}");

        let redis_manager = RedisConnectionManager::new(redis_uri)?;
        let redis = Pool::builder().max_size(8).build(redis_manager).await?;

        // Connect to osu! API
        let osu_client_id = config.tokens.osu_client_id;
        let osu_client_secret = &config.tokens.osu_client_secret;
        let osu = Osu::new(osu_client_id, osu_client_secret).await?;

        // Log custom client into osu!
        let custom = CustomClient::new(config).await?;

        let data = ContextData::new(&psql, application_id).await?;
        let (cache, resume_data) = Cache::new(&redis).await;
        let stats = Arc::new(BotStats::new(osu.metrics()));

        if !resume_data.is_empty() {
            stats.populate(&cache);
        }

        let clients = Clients::new(psql, redis, osu, custom);
        let (cluster, events) =
            build_cluster(discord_token, Arc::clone(&http), resume_data).await?;

        let ctx = Self {
            cache,
            stats,
            http,
            clients,
            cluster,
            data,
            standby: Standby::new(),
            auth_standby: AuthenticationStandby::default(),
            buckets: Buckets::new(),
            member_requests: MemberRequests::new(tx),
        };

        Ok((ctx, events))
    }
}

pub struct MemberRequests {
    pub tx: UnboundedSender<(Id<GuildMarker>, u64)>,
    pub todo_guilds: DashSet<Id<GuildMarker>>,
}

impl MemberRequests {
    fn new(tx: UnboundedSender<(Id<GuildMarker>, u64)>) -> Self {
        Self {
            tx,
            todo_guilds: DashSet::new(),
        }
    }
}

struct Clients {
    custom: CustomClient,
    osu: Osu,
    psql: Database,
    redis: Redis,
}

impl Clients {
    fn new(psql: Database, redis: Redis, osu: Osu, custom: CustomClient) -> Self {
        Self {
            psql,
            redis,
            osu,
            custom,
        }
    }
}

struct ContextData {
    application_id: Id<ApplicationMarker>,
    games: Games,
    guilds: FlurryMap<Id<GuildMarker>, GuildConfig>, // read-heavy
    map_garbage_collection: Mutex<HashSet<NonZeroU32>>,
    matchlive: MatchLiveChannels,
    msgs_to_process: DashSet<Id<MessageMarker>>,
    osu_tracking: OsuTracking,
    role_assigns: FlurryMap<(u64, u64), AssignRoles>, // read-heavy
    snipe_countries: FlurryMap<CountryCode, String>,  // read-heavy
    tracked_streams: FlurryMap<u64, Vec<u64>>,        // read-heavy
}

impl ContextData {
    async fn new(psql: &Database, application_id: Id<ApplicationMarker>) -> BotResult<Self> {
        Ok(Self {
            application_id,
            games: Games::default(),
            guilds: psql.get_guilds().await?,
            map_garbage_collection: Mutex::new(HashSet::new()),
            matchlive: MatchLiveChannels::new(),
            msgs_to_process: DashSet::new(),
            osu_tracking: OsuTracking::new(psql).await?,
            role_assigns: psql.get_role_assigns().await?,
            snipe_countries: psql.get_snipe_countries().await?,
            tracked_streams: psql.get_stream_tracks().await?,
        })
    }
}

#[derive(Default)]
struct Games {
    bg: BgGames,
    hl: HlGames,
    hl_retries: HlRetries,
}

type BgGames = RwLock<HashMap<Id<ChannelMarker>, BgGameState>>;
type HlGames = TokioMutex<HashMap<Id<UserMarker>, HlGameState>>;
type HlRetries = DashMap<Id<MessageMarker>, RetryState>;
