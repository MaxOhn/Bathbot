use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock, RwLock},
    time::Duration,
};

use bathbot_cache::Cache;
use bathbot_client::Client as BathbotClient;
use bathbot_model::twilight::id::{ArchivedId, IdRkyvMap};
use bathbot_psql::{Database, model::configs::GuildConfig};
use bathbot_util::{BucketName, Buckets, IntHasher, MetricsReader};
use eyre::{Result, WrapErr};
use flexmap::{std::StdMutexMap, tokio::TokioRwLockMap};
use metrics_util::layers::{FanoutBuilder, Layer, PrefixLayer};
use papaya::HashMap as PapayaMap;
use rkyv::{vec::ArchivedVec, with::ArchiveWith};
use rosu_v2::Osu;
use shutdown::CacheGuildShards;
use time::OffsetDateTime;
use tokio::sync::{Mutex as TokioMutex, mpsc::UnboundedSender};
use twilight_gateway::{MessageSender, Shard};
use twilight_http::{Client, client::InteractionClient};
use twilight_model::id::{
    Id,
    marker::{ApplicationMarker, ChannelMarker, GuildMarker, UserMarker},
};
use twilight_standby::Standby;

use self::osutrack::OsuTrackUserNotifTimestamps;
use super::{BotConfig, BotMetrics};
use crate::{
    active::{ActiveMessages, impls::BackgroundGame},
    tracking::{Ordr, OsuTracking, ScoresWebSocket, ScoresWebSocketDisconnect},
};

mod discord;
mod games;
mod manager;
mod messages;
mod osutrack;
mod set_commands;
mod shutdown;

#[cfg(feature = "matchlive")]
mod matchlive;

#[cfg(feature = "twitchtracking")]
mod twitch;

type GuildShards = PapayaMap<Id<GuildMarker>, u32>;
type GuildConfigs = PapayaMap<Id<GuildMarker>, GuildConfig, IntHasher>;
type MissAnalyzerGuilds = RwLock<HashSet<Id<GuildMarker>, IntHasher>>;

#[cfg(feature = "twitchtracking")]
type TrackedStreams = PapayaMap<u64, Vec<Id<ChannelMarker>>, IntHasher>;

static CONTEXT: OnceLock<Box<Context>> = OnceLock::new();

pub struct Context {
    pub buckets: Buckets,
    pub shard_senders: RwLock<HashMap<u32, MessageSender, IntHasher>>,
    pub member_requests: MemberRequests,
    pub active_msgs: ActiveMessages,
    pub start_time: OffsetDateTime,
    pub metrics: MetricsReader,
    data: ContextData,
    clients: Clients,

    /// Keeps track of the amount of times content was added to a usual bot
    /// response to remind users about the new /builder command.
    pub builder_notices: StdMutexMap<Id<UserMarker>, usize, IntHasher>,
    /// Notify the scores websocket when it should initiate a disconnect
    scores_ws_disconnect: Mutex<Option<ScoresWebSocketDisconnect>>,
}

impl Context {
    #[track_caller]
    pub fn get() -> &'static Self {
        CONTEXT.get().expect("Context not yet initialized")
    }

    pub fn interaction() -> InteractionClient<'static> {
        let ctx = Self::get();

        ctx.clients.http.interaction(ctx.data.application_id)
    }

    pub fn http() -> &'static Client {
        &Self::get().clients.http
    }

    pub fn standby() -> &'static Standby {
        &Self::get().clients.standby
    }

    pub fn cache() -> &'static Cache {
        &Self::get().data.cache
    }

    pub fn osu() -> &'static Osu {
        &Self::get().clients.osu
    }

    pub fn client() -> &'static BathbotClient {
        &Self::get().clients.custom
    }

    pub fn ordr_available() -> bool {
        Self::get().clients.ordr.is_some()
    }

    pub fn try_ordr() -> Option<&'static Ordr> {
        Self::get().clients.ordr.as_deref()
    }

    /// Panics if ordr is not available
    #[track_caller]
    pub fn ordr() -> &'static Ordr {
        Self::get()
            .clients
            .ordr
            .as_deref()
            .expect("ordr unavailable")
    }

    pub fn psql() -> &'static Database {
        &Self::get().clients.psql
    }

    pub fn tracking() -> &'static OsuTracking {
        &Self::get().data.osu_tracking
    }

    #[cfg(feature = "server")]
    pub fn auth_standby() -> &'static bathbot_server::AuthenticationStandby {
        &Self::get().clients.auth_standby
    }

    pub fn guild_shards(&self) -> &GuildShards {
        &self.data.guild_shards
    }

    pub fn miss_analyzer_guilds() -> &'static MissAnalyzerGuilds {
        &Self::get().data.miss_analyzer_guilds
    }

    pub fn has_miss_analyzer(guild: &Id<GuildMarker>) -> bool {
        Self::miss_analyzer_guilds().read().unwrap().contains(guild)
    }

    #[cfg(feature = "twitch")]
    pub fn online_twitch_streams() -> &'static crate::tracking::OnlineTwitchStreams {
        &Self::get().data.online_twitch_streams
    }

    pub async fn init(tx: UnboundedSender<(Id<GuildMarker>, u32)>) -> Result<ContextResult> {
        let (_prometheus, reader) = {
            const DEFAULT_BUCKETS: [f64; 10] =
                [0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0];

            let prometheus = metrics_exporter_prometheus::PrometheusBuilder::new()
                .set_buckets(&DEFAULT_BUCKETS)
                .expect("DEFAULT_BUCKETS is not empty")
                .build_recorder();

            let prometheus_handle = prometheus.handle();

            let reader = MetricsReader::new();

            let fanout = FanoutBuilder::default()
                .add_recorder(prometheus)
                .add_recorder(reader.clone())
                .build();

            let prefix = PrefixLayer::new("bathbot").layer(fanout);

            metrics::set_global_recorder(prefix)
                .map_err(|e| eyre!("Failed to install metrics recorder: {e:?}"))?;

            (prometheus_handle, reader)
        };

        let start_time = OffsetDateTime::now_utc();

        let config = BotConfig::get();

        // Connect to psql database
        let psql =
            Database::new(&config.database_url).wrap_err("Failed to create database client")?;

        // Connect to discord API
        let (http, application_id) = discord::http(config)
            .await
            .wrap_err("Failed to create discord http client")?;

        // Connect to osu! API
        let osu_client_id = config.tokens.osu_client_id;
        let osu_client_secret = &config.tokens.osu_client_secret;
        let osu = Osu::new(osu_client_id, osu_client_secret.as_ref())
            .await
            .wrap_err("Failed to create osu client")?;

        let cache = Cache::new(&config.redis_host, config.redis_port, config.redis_db_idx)
            .await
            .wrap_err("Failed to create redis cache")?;

        let data = ContextData::new(&psql, cache, application_id)
            .await
            .wrap_err("Failed to create context data")?;

        let resume_data = data
            .cache
            .defrost()
            .await
            .wrap_err("Failed to defrost cache")?
            .unwrap_or_default();

        BotMetrics::init(&data.cache);

        let client_fut = BathbotClient::new(
            #[cfg(feature = "twitch")]
            (&config.tokens.twitch_client_id, &config.tokens.twitch_token),
            &config.tokens.github_token,
        );

        let custom_client = client_fut
            .await
            .wrap_err("Failed to create custom client")?;

        let ordr_fut = Ordr::new(
            #[cfg(not(debug_assertions))]
            config.tokens.ordr_key.as_ref(),
        );

        let ordr = match tokio::time::timeout(Duration::from_secs(20), ordr_fut).await {
            Ok(Ok(ordr)) => Some(Arc::new(ordr)),
            Ok(Err(err)) => return Err(err),
            Err(_) => {
                warn!("o!rdr timed out, initializing without it");

                None
            }
        };

        let shards_iter = discord::gateway(config, &http, resume_data)
            .await
            .wrap_err("Failed to create discord gateway shards")?;

        let mut shard_senders = HashMap::default();
        let mut shards = Vec::new();

        for shard in shards_iter {
            shard_senders.insert(shard.id().number(), shard.sender());
            shards.push(Arc::new(TokioMutex::new(shard)));
        }

        let shard_senders = RwLock::new(shard_senders);

        #[cfg(feature = "server")]
        let (auth_standby, server_tx) = bathbot_server(config, _prometheus, reader.clone())
            .await
            .wrap_err("Failed to create server")?;

        let clients = Clients {
            http,
            standby: Standby::new(),
            custom: custom_client,
            osu,
            psql,
            ordr,
            #[cfg(feature = "server")]
            auth_standby,
        };

        let ctx = Self {
            clients,
            shard_senders,
            data,
            buckets: Buckets::new(),
            member_requests: MemberRequests::new(tx),
            active_msgs: ActiveMessages::new(),
            scores_ws_disconnect: Mutex::new(None),
            start_time,
            metrics: reader,
            builder_notices: StdMutexMap::default(),
        };

        if CONTEXT.set(Box::new(ctx)).is_err() {
            panic!("must init Context only once");
        }

        // Some websocket functionality relies on `Context::get` being
        // available so we should connect only after setting the context.
        match ScoresWebSocket::connect().await {
            Ok(disconnect) => {
                *Self::get().scores_ws_disconnect.lock().unwrap() = Some(disconnect);
            }
            Err(err) => warn!(?err, "Failed to connect scores websocket"),
        };

        Ok((
            shards,
            #[cfg(feature = "server")]
            server_tx,
        ))
    }

    /// Acquire an entry for the user in the bucket and optionally return the
    /// cooldown in amount of seconds if acquiring the entry was ratelimitted.
    pub fn check_ratelimit(user_id: Id<UserMarker>, bucket: BucketName) -> Option<i64> {
        let ratelimit = Self::get()
            .buckets
            .get(bucket)
            .lock()
            .unwrap()
            .take(user_id.get());

        (ratelimit > 0).then_some(ratelimit)
    }
}

type Shards = Vec<Arc<TokioMutex<Shard>>>;

#[cfg(not(feature = "server"))]
pub type ContextResult = (Shards,);

#[cfg(feature = "server")]
pub type ContextResult = (Shards, tokio::sync::oneshot::Sender<()>);

pub struct MemberRequests {
    pub tx: UnboundedSender<(Id<GuildMarker>, u32)>,
    pub pending_guilds: Mutex<HashSet<Id<GuildMarker>, IntHasher>>,
}

impl MemberRequests {
    fn new(tx: UnboundedSender<(Id<GuildMarker>, u32)>) -> Self {
        Self {
            tx,
            pending_guilds: Mutex::new(HashSet::default()),
        }
    }
}

struct Clients {
    http: Arc<Client>,
    standby: Standby,
    custom: BathbotClient,
    osu: Osu,
    psql: Database,
    ordr: Option<Arc<Ordr>>,
    #[cfg(feature = "server")]
    auth_standby: Arc<bathbot_server::AuthenticationStandby>,
}

struct ContextData {
    cache: Cache,
    application_id: Id<ApplicationMarker>,
    games: Games,
    #[cfg(feature = "matchlive")]
    matchlive: crate::matchlive::MatchLiveChannels,
    #[cfg(feature = "twitchtracking")]
    tracked_streams: TrackedStreams,
    osu_tracking: OsuTracking,
    guild_configs: GuildConfigs,
    guild_shards: GuildShards,
    miss_analyzer_guilds: MissAnalyzerGuilds,
    osutrack_user_notif_timestamps: OsuTrackUserNotifTimestamps,
    #[cfg(feature = "twitch")]
    online_twitch_streams: crate::tracking::OnlineTwitchStreams,
}

impl ContextData {
    async fn new(
        psql: &Database,
        cache: Cache,
        application_id: Id<ApplicationMarker>,
    ) -> Result<Self> {
        #[cfg(feature = "twitchtracking")]
        let (
            guild_configs_res,
            tracked_streams_res,
            guild_shards,
            miss_analyzer_guilds,
            osu_tracking,
        ) = tokio::join!(
            psql.select_guild_configs::<IntHasher>(),
            psql.select_tracked_twitch_streams::<IntHasher>(),
            Self::fetch_guild_shards(&cache),
            Self::fetch_miss_analyzer_guilds(&cache),
            OsuTracking::new(psql),
        );

        #[cfg(not(feature = "twitchtracking"))]
        let (guild_configs_res, guild_shards, miss_analyzer_guilds, osu_tracking) = tokio::join!(
            psql.select_guild_configs::<IntHasher>(),
            Self::fetch_guild_shards(&cache),
            Self::fetch_miss_analyzer_guilds(&cache),
            OsuTracking::new(psql)
        );

        Ok(Self {
            cache,
            guild_configs: guild_configs_res
                .wrap_err("Failed to get guild configs")?
                .into_iter()
                .collect(),
            #[cfg(feature = "twitchtracking")]
            tracked_streams: tracked_streams_res
                .wrap_err("Failed to get tracked streams")?
                .into_iter()
                .collect(),
            osu_tracking: osu_tracking.wrap_err("Failed to create osu! tracking")?,
            application_id,
            games: Games::new(),
            guild_shards,
            #[cfg(feature = "matchlive")]
            matchlive: crate::matchlive::MatchLiveChannels::new(),
            miss_analyzer_guilds,
            osutrack_user_notif_timestamps: OsuTrackUserNotifTimestamps::default(),
            #[cfg(feature = "twitch")]
            online_twitch_streams: crate::tracking::OnlineTwitchStreams::default(),
        })
    }

    async fn fetch_guild_shards(cache: &Cache) -> GuildShards {
        let fetch_fut = cache
            .fetch::<_, <CacheGuildShards as ArchiveWith<[(Id<GuildMarker>, u32)]>>::Archived>(
                "guild_shards",
            );

        match fetch_fut.await {
            Ok(Ok(guild_shards)) => guild_shards
                .deserialize_with::<CacheGuildShards, _>()
                .unwrap()
                .into_iter()
                .map(|entry| (entry.key, entry.value))
                .collect(),
            Ok(Err(_)) => GuildShards::default(),
            Err(err) => {
                warn!(?err, "Failed to fetch guild shards, creating default...");

                GuildShards::default()
            }
        }
    }

    async fn fetch_miss_analyzer_guilds(cache: &Cache) -> MissAnalyzerGuilds {
        let fetch_fut =
            cache.fetch::<_, ArchivedVec<ArchivedId<GuildMarker>>>("miss_analyzer_guilds");

        match fetch_fut.await {
            Ok(Ok(miss_analyzer_guilds)) => RwLock::new(
                miss_analyzer_guilds
                    .deserialize_with::<IdRkyvMap, Vec<_>>()
                    .unwrap()
                    .into_iter()
                    .collect(),
            ),
            Ok(Err(_)) => MissAnalyzerGuilds::default(),
            Err(err) => {
                warn!(
                    ?err,
                    "Failed to fetch miss analyzer guilds, creating default..."
                );

                MissAnalyzerGuilds::default()
            }
        }
    }
}

type BgGames = TokioRwLockMap<Id<ChannelMarker>, BackgroundGame, IntHasher>;

struct Games {
    bg: BgGames,
}

impl Games {
    fn new() -> Self {
        Self {
            bg: BgGames::with_shard_amount_and_hasher(16, IntHasher),
        }
    }
}

#[cfg(feature = "server")]
async fn bathbot_server(
    config: &BotConfig,
    prometheus: metrics_exporter_prometheus::PrometheusHandle,
    metrics_reader: MetricsReader,
) -> Result<(
    Arc<bathbot_server::AuthenticationStandby>,
    tokio::sync::oneshot::Sender<()>,
)> {
    let builder = bathbot_server::AppStateBuilder {
        website_path: config.paths.website.clone(),
        prometheus,
        metrics_reader,
        osu_client_id: config.tokens.osu_client_id,
        osu_client_secret: config.tokens.osu_client_secret.to_string(),
        twitch_client_id: config.tokens.twitch_client_id.to_string(),
        twitch_token: config.tokens.twitch_token.to_string(),
        redirect_base: config.server.public_url.to_string(),
    };

    let (server, standby, tx) = bathbot_server::Server::new(builder)?;

    tokio::spawn(server.run(config.server.port));

    Ok((standby, tx))
}
