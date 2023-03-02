use std::{collections::HashMap, sync::Arc};

use bathbot_cache::Cache;
use bathbot_client::Client as BathbotClient;
use bathbot_psql::{model::configs::GuildConfig, Database};
use bathbot_util::IntHasher;
use eyre::{Result, WrapErr};
use flexmap::{
    std::StdMutexMap,
    tokio::{TokioMutexMap, TokioRwLockMap},
};
use flurry::HashMap as FlurryMap;
use futures::{future, stream::FuturesUnordered, FutureExt, StreamExt};
use hashbrown::HashSet;
use parking_lot::Mutex;
use rosu_v2::Osu;
use tokio::sync::mpsc::UnboundedSender;
use twilight_gateway::{
    stream, CloseFrame, Config, ConfigBuilder, EventTypeFlags, Intents, MessageSender, Session,
    Shard, ShardId,
};
use twilight_http::{client::InteractionClient, Client};
use twilight_model::{
    channel::message::AllowedMentions,
    gateway::{
        payload::outgoing::update_presence::UpdatePresencePayload,
        presence::{ActivityType, MinimalActivity, Status},
    },
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

use super::{buckets::Buckets, BotStats};

mod games;
mod manager;
mod matchlive;
mod messages;
mod shutdown;
mod twitch;

type GuildShards = FlurryMap<Id<GuildMarker>, u64>;
type GuildConfigs = FlurryMap<Id<GuildMarker>, GuildConfig, IntHasher>;
type TrackedStreams = FlurryMap<u64, Vec<Id<ChannelMarker>>, IntHasher>;

pub struct Context {
    #[cfg(feature = "server")]
    pub auth_standby: Arc<bathbot_server::AuthenticationStandby>,
    pub buckets: Buckets,
    pub cache: Cache,
    pub shard_senders: HashMap<u64, MessageSender>,
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

        // Connect to psql database
        let psql =
            Database::new(&config.database_url).wrap_err("Failed to create database client")?;

        // Connect to discord API
        let (http, application_id) = discord_http(config)
            .await
            .wrap_err("Failed to create discord http client")?;

        // Connect to osu! API
        let osu_client_id = config.tokens.osu_client_id;
        let osu_client_secret = &config.tokens.osu_client_secret;
        let osu = Osu::new(osu_client_id, osu_client_secret.as_ref())
            .await
            .wrap_err("Failed to create osu client")?;

        let data = ContextData::new(&psql, application_id)
            .await
            .wrap_err("Failed to create context data")?;

        let cache = Cache::new(&config.redis_host, config.redis_port, config.redis_db_idx)
            .await
            .wrap_err("Failed to create redis cache")?;

        let resume_data = cache.defrost().await.wrap_err("Failed to defrost cache")?;

        let (stats, registry) = BotStats::new(osu.metrics());

        if !resume_data.is_empty() {
            stats.populate(&cache).await;
        }

        let client_fut = BathbotClient::new(
            &config.tokens.osu_session,
            #[cfg(feature = "twitch")]
            (&config.tokens.twitch_client_id, &config.tokens.twitch_token),
            &registry,
        );

        let custom_client = client_fut
            .await
            .wrap_err("Failed to create custom client")?;

        let clients = Clients::new(psql, osu, custom_client);

        let shards = discord_gateway(config, &http, resume_data)
            .await
            .wrap_err("Failed to create discord gateway shards")?;

        let shard_senders: HashMap<_, _> = shards
            .iter()
            .map(|shard| (shard.id().number(), shard.sender()))
            .collect();

        #[cfg(feature = "server")]
        let (auth_standby, server_tx) = bathbot_server(&config, registry, &stats)
            .await
            .wrap_err("Failed to create server")?;

        let ctx = Self {
            cache,
            stats: Arc::new(stats),
            http,
            clients,
            shard_senders,
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
            shards,
            #[cfg(feature = "server")]
            server_tx,
        ))
    }

    pub async fn down_resumable(shards: &mut [Shard]) -> HashMap<u64, Session> {
        shards
            .iter_mut()
            .map(|shard| {
                let shard_id = shard.id().number();

                shard
                    .close(CloseFrame::RESUME)
                    .map(move |res| (shard_id, res))
            })
            .collect::<FuturesUnordered<_>>()
            .filter_map(|(shard_id, res)| match res {
                Ok(opt) => future::ready(opt.map(|session| (shard_id, session))),
                Err(err) => {
                    warn!("Failed to close shard {shard_id}: {err}");

                    future::ready(None)
                }
            })
            .collect()
            .await
    }
}

#[cfg(not(feature = "server"))]
pub type ContextTuple = (Context, Vec<Shard>);

#[cfg(feature = "server")]
pub type ContextTuple = (Context, Vec<Shard>, tokio::sync::oneshot::Sender<()>);

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
}

impl Clients {
    fn new(psql: Database, osu: Osu, custom: BathbotClient) -> Self {
        Self { psql, osu, custom }
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
        let (guild_configs_res, tracked_streams_res) = tokio::join!(
            psql.select_guild_configs::<IntHasher>(),
            psql.select_tracked_twitch_streams::<IntHasher>(),
        );

        Ok(Self {
            guild_configs: guild_configs_res
                .wrap_err("Failed to get guild configs")?
                .into_iter()
                .collect(),
            tracked_streams: tracked_streams_res
                .wrap_err("Failed to get tracked streams")?
                .into_iter()
                .collect(),
            application_id,
            games: Games::new(),
            guild_shards: GuildShards::default(),
            #[cfg(feature = "matchlive")]
            matchlive: crate::matchlive::MatchLiveChannels::new(),
            msgs_to_process: Mutex::new(HashSet::default()),
            #[cfg(feature = "osutracking")]
            osu_tracking: crate::tracking::OsuTracking::new(OsuTrackingManager::new(psql))
                .await
                .wrap_err("Failed to create osu tracking")?,
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

async fn discord_http(config: &BotConfig) -> Result<(Arc<Client>, Id<ApplicationMarker>)> {
    let mentions = AllowedMentions {
        replied_user: true,
        ..Default::default()
    };

    // Connect to the discord http client
    let http = Client::builder()
        .token(config.tokens.discord.to_string())
        .remember_invalid_token(false)
        .default_allowed_mentions(mentions)
        .build();

    let http = Arc::new(http);

    let current_user = http
        .current_user()
        .await
        .wrap_err("Failed to get current user")?
        .model()
        .await
        .wrap_err("Failed to deserialize current user")?;

    let application_id = current_user.id.cast();

    info!(
        "Connecting to Discord as {}#{:04}...",
        current_user.name, current_user.discriminator
    );

    Ok((http, application_id))
}

async fn discord_gateway(
    config: &BotConfig,
    http: &Client,
    resume_data: HashMap<u64, Session>,
) -> Result<Vec<Shard>> {
    let intents = Intents::GUILDS
        | Intents::GUILD_MEMBERS
        | Intents::GUILD_MESSAGES
        | Intents::DIRECT_MESSAGES
        | Intents::MESSAGE_CONTENT;

    let event_types = EventTypeFlags::CHANNEL_CREATE
        | EventTypeFlags::CHANNEL_DELETE
        | EventTypeFlags::CHANNEL_UPDATE
        | EventTypeFlags::GUILD_CREATE
        | EventTypeFlags::GUILD_DELETE
        | EventTypeFlags::GUILD_UPDATE
        | EventTypeFlags::INTERACTION_CREATE
        | EventTypeFlags::MEMBER_ADD
        | EventTypeFlags::MEMBER_REMOVE
        | EventTypeFlags::MEMBER_UPDATE
        | EventTypeFlags::MEMBER_CHUNK
        | EventTypeFlags::MESSAGE_CREATE
        | EventTypeFlags::MESSAGE_DELETE
        | EventTypeFlags::MESSAGE_DELETE_BULK
        | EventTypeFlags::READY
        | EventTypeFlags::ROLE_CREATE
        | EventTypeFlags::ROLE_DELETE
        | EventTypeFlags::ROLE_UPDATE
        | EventTypeFlags::THREAD_CREATE
        | EventTypeFlags::THREAD_DELETE
        | EventTypeFlags::THREAD_UPDATE
        | EventTypeFlags::UNAVAILABLE_GUILD
        | EventTypeFlags::USER_UPDATE;

    let activity = MinimalActivity {
        kind: ActivityType::Playing,
        name: "osu!".to_owned(),
        url: None,
    };

    let presence =
        UpdatePresencePayload::new([activity.into()], false, None, Status::Online).unwrap();

    let config = Config::builder(config.tokens.discord.to_string(), intents)
        .event_types(event_types)
        .large_threshold(250)
        .presence(presence)
        .build();

    let config_callback =
        |shard_id: ShardId, builder: ConfigBuilder| match resume_data.get(&shard_id.number()) {
            Some(session) => builder.session(session.to_owned()).build(),
            None => builder.build(),
        };

    stream::create_recommended(http, config, config_callback)
        .await
        .map(Iterator::collect)
        .wrap_err("Failed to create recommended shards")
}

#[cfg(feature = "server")]
async fn bathbot_server(
    config: &BotConfig,
    registry: prometheus::Registry,
    stats: &BotStats,
) -> Result<(
    Arc<bathbot_server::AuthenticationStandby>,
    tokio::sync::oneshot::Sender<()>,
)> {
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

    let (server, standby, tx) = bathbot_server::Server::new(builder)?;

    tokio::spawn(server.run(config.server.port));

    Ok((standby, tx))
}
