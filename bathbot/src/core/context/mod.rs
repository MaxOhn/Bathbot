use std::sync::Arc;

use bathbot_client::Client as BathbotClient;
use bathbot_psql::{model::configs::GuildConfig, Database};
use bathbot_util::IntHasher;
use bb8_redis::{bb8::Pool, RedisConnectionManager};
use eyre::{Result, WrapErr};
use flexmap::{
    std::StdMutexMap,
    tokio::{TokioMutexMap, TokioRwLockMap},
};
use flurry::HashMap as FlurryMap;
use hashbrown::HashSet;
use parking_lot::Mutex;
use rosu_v2::Osu;
use tokio::sync::mpsc::UnboundedSender;
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
    core::BotConfig,
    games::{
        bg::GameState as BgGameState,
        hl::{retry::RetryState, GameState as HlGameState},
    },
    pagination::Pagination,
};

#[cfg(feature = "osutracking")]
use crate::manager::OsuTrackingManager;

use super::{buckets::Buckets, cluster::build_cluster, BotStats, Cache};

mod games;
mod manager;
mod matchlive;
mod messages;
mod shutdown;
mod twitch;

pub type Redis = Pool<RedisConnectionManager>;

type GuildShards = FlurryMap<Id<GuildMarker>, u64>;
type GuildConfigs = FlurryMap<Id<GuildMarker>, GuildConfig, IntHasher>;
type TrackedStreams = FlurryMap<u64, Vec<Id<ChannelMarker>>, IntHasher>;

pub struct Context {
    #[cfg(feature = "server")]
    pub auth_standby: Arc<bathbot_server::AuthenticationStandby>,
    pub buckets: Buckets,
    pub cache: Cache,
    pub cluster: Cluster,
    pub http: Arc<Client>,
    pub member_requests: MemberRequests,
    pub paginations: Arc<TokioMutexMap<Id<MessageMarker>, Pagination, IntHasher>>,
    pub standby: Standby,
    pub stats: Arc<BotStats>,
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

    /// Returns the custom client
    pub fn client(&self) -> &BathbotClient {
        &self.clients.custom
    }

    #[cfg(feature = "osutracking")]
    pub fn tracking(&self) -> &crate::tracking::OsuTracking {
        &self.data.osu_tracking
    }

    pub fn guild_shards(&self) -> &GuildShards {
        &self.data.guild_shards
    }

    pub async fn new(tx: UnboundedSender<(Id<GuildMarker>, u64)>) -> Result<ContextTuple> {
        let config = BotConfig::get();
        let discord_token = &config.tokens.discord;

        let mentions = AllowedMentionsBuilder::new()
            .replied_user()
            .roles()
            .users()
            .build();

        // Connect to the discord http client
        let http = Client::builder()
            .token(discord_token.to_string())
            .remember_invalid_token(false)
            .default_allowed_mentions(mentions)
            .build();

        let http = Arc::new(http);

        let current_user = http
            .current_user()
            .exec()
            .await
            .wrap_err("failed to get current user")?
            .model()
            .await
            .wrap_err("failed to deserialize current user")?;

        let application_id = current_user.id.cast();

        info!(
            "Connecting to Discord as {}#{}...",
            current_user.name, current_user.discriminator
        );

        // Connect to psql database
        let psql =
            Database::new(&config.database_url).wrap_err("failed to create database client")?;

        // Connect to redis
        let redis_host = &config.redis_host;
        let redis_port = config.redis_port;
        let redis_uri = format!("redis://{redis_host}:{redis_port}");

        let redis_manager =
            RedisConnectionManager::new(redis_uri).wrap_err("failed to create redis manager")?;

        let redis = Pool::builder()
            .max_size(8)
            .build(redis_manager)
            .await
            .wrap_err("failed to create redis pool")?;

        // Connect to osu! API
        let osu_client_id = config.tokens.osu_client_id;
        let osu_client_secret = &config.tokens.osu_client_secret;
        let osu = Osu::new(osu_client_id, osu_client_secret.as_ref())
            .await
            .wrap_err("failed to create osu client")?;

        let data = ContextData::new(&psql, application_id)
            .await
            .wrap_err("failed to create context data")?;

        let (cache, resume_data) = Cache::new(&redis).await;
        let (stats, registry) = BotStats::new(osu.metrics());

        let client_fut = BathbotClient::new(
            &config.tokens.osu_session,
            #[cfg(feature = "twitch")]
            (&config.tokens.twitch_client_id, &config.tokens.twitch_token),
            &registry,
        );

        let custom_client = client_fut
            .await
            .wrap_err("failed to create custom client")?;

        if !resume_data.is_empty() {
            stats.populate(&cache);
        }

        let clients = Clients::new(psql, redis, osu, custom_client);

        let (cluster, events) = build_cluster(discord_token, Arc::clone(&http), resume_data)
            .await
            .wrap_err("failed to build cluster")?;

        #[cfg(feature = "server")]
        let (auth_standby, server_tx) = {
            let builder = bathbot_server::AppStateBuilder {
                website_path: config.paths.website.clone(),
                metrics: registry,
                guild_counter: stats.cache_counts.guilds.clone(),
                osu_client_id: config.tokens.osu_client_id,
                osu_client_secret: config.tokens.osu_client_secret.clone(),
                twitch_client_id: config.tokens.twitch_client_id.clone(),
                twitch_token: config.tokens.twitch_token.clone(),
                redirect_base: config.server.public_url.clone(),
            };

            let (server, standby, tx) =
                bathbot_server::Server::new(builder).wrap_err("failed to create server")?;

            tokio::spawn(server.run(config.server.port));

            (standby, tx)
        };

        let ctx = Self {
            cache,
            stats: Arc::new(stats),
            http,
            clients,
            cluster,
            data,
            standby: Standby::new(),
            #[cfg(feature = "server")]
            auth_standby,
            buckets: Buckets::new(),
            member_requests: MemberRequests::new(tx),
            paginations: Arc::new(TokioMutexMap::with_shard_amount_and_hasher(16, IntHasher)),
        };

        Ok((
            ctx,
            events,
            #[cfg(feature = "server")]
            server_tx,
        ))
    }
}

#[cfg(not(feature = "server"))]
pub type ContextTuple = (Context, Events);

#[cfg(feature = "server")]
pub type ContextTuple = (Context, Events, tokio::sync::oneshot::Sender<()>);

pub struct MemberRequests {
    pub tx: UnboundedSender<(Id<GuildMarker>, u64)>,
    pub todo_guilds: Mutex<HashSet<Id<GuildMarker>, IntHasher>>,
}

impl MemberRequests {
    fn new(tx: UnboundedSender<(Id<GuildMarker>, u64)>) -> Self {
        Self {
            tx,
            todo_guilds: Mutex::new(HashSet::default()),
        }
    }
}

struct Clients {
    custom: BathbotClient,
    osu: Osu,
    psql: Database,
    redis: Redis,
}

impl Clients {
    fn new(psql: Database, redis: Redis, osu: Osu, custom: BathbotClient) -> Self {
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
    msgs_to_process: Mutex<HashSet<Id<MessageMarker>, IntHasher>>,
    #[cfg(feature = "matchlive")]
    matchlive: crate::matchlive::MatchLiveChannels,
    #[cfg(feature = "osutracking")]
    osu_tracking: crate::tracking::OsuTracking,
    guild_configs: GuildConfigs,     // read-heavy
    tracked_streams: TrackedStreams, // read-heavy
    guild_shards: GuildShards,       // necessary to request members for a guild
}

impl ContextData {
    async fn new(psql: &Database, application_id: Id<ApplicationMarker>) -> Result<Self> {
        Ok(Self {
            application_id,
            games: Games::new(),
            guild_shards: GuildShards::default(),
            guild_configs: psql
                .select_guild_configs::<IntHasher>()
                .await
                .wrap_err("failed to get guild configs")?
                .into_iter()
                .collect(),
            #[cfg(feature = "matchlive")]
            matchlive: crate::matchlive::MatchLiveChannels::new(),
            msgs_to_process: Mutex::new(HashSet::default()),
            #[cfg(feature = "osutracking")]
            osu_tracking: crate::tracking::OsuTracking::new(OsuTrackingManager::new(psql))
                .await
                .wrap_err("failed to create osu tracking")?,
            tracked_streams: psql
                .select_tracked_twitch_streams::<IntHasher>()
                .await
                .wrap_err("failed to get tracked streams")?
                .into_iter()
                .collect(),
        })
    }
}

struct Games {
    bg: BgGames,
    hl: HlGames,
    hl_retries: HlRetries,
}

impl Games {
    fn new() -> Self {
        Self {
            bg: BgGames::with_shard_amount_and_hasher(16, IntHasher),
            hl: HlGames::with_shard_amount_and_hasher(16, IntHasher),
            hl_retries: HlRetries::with_shard_amount_and_hasher(4, IntHasher),
        }
    }
}

type BgGames = TokioRwLockMap<Id<ChannelMarker>, BgGameState, IntHasher>;
type HlGames = TokioMutexMap<Id<UserMarker>, HlGameState, IntHasher>;
type HlRetries = StdMutexMap<Id<MessageMarker>, RetryState, IntHasher>;
