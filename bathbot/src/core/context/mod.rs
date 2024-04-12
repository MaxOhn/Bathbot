use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

use bathbot_cache::Cache;
use bathbot_client::Client as BathbotClient;
use bathbot_model::twilight_model::id::IdRkyv;
use bathbot_psql::{model::configs::GuildConfig, Database};
use bathbot_util::{IntHasher, MetricsReader};
use eyre::{Result, WrapErr};
use flexmap::tokio::TokioRwLockMap;
use flurry::{HashMap as FlurryMap, HashSet as FlurrySet};
use futures::{future, stream::FuturesUnordered, FutureExt, StreamExt};
use hashbrown::HashSet;
use metrics_util::layers::{FanoutBuilder, Layer, PrefixLayer};
use rkyv::with::With;
use rosu_v2::Osu;
use time::OffsetDateTime;
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
        marker::{ApplicationMarker, ChannelMarker, GuildMarker, UserMarker},
        Id,
    },
};
use twilight_standby::Standby;

pub use self::ext::ContextExt;
use self::osutrack::OsuTrackUserNotifTimestamps;
use super::{
    buckets::{BucketName, Buckets},
    BotConfig, BotMetrics,
};
use crate::{
    active::{impls::BackgroundGame, ActiveMessages},
    tracking::Ordr,
};

mod ext;
mod games;
mod manager;
mod matchlive;
mod messages;
mod osutrack;
mod shutdown;
mod twitch;

type GuildShards = FlurryMap<Id<GuildMarker>, u64>;
type GuildConfigs = FlurryMap<Id<GuildMarker>, GuildConfig, IntHasher>;
type TrackedStreams = FlurryMap<u64, Vec<Id<ChannelMarker>>, IntHasher>;
type MissAnalyzerGuilds = FlurrySet<Id<GuildMarker>, IntHasher>;

pub struct Context {
    #[cfg(feature = "server")]
    pub auth_standby: Arc<bathbot_server::AuthenticationStandby>,
    pub buckets: Buckets,
    pub cache: Cache,
    pub shard_senders: RwLock<HashMap<u64, MessageSender>>,
    pub http: Arc<Client>,
    pub member_requests: MemberRequests,
    pub active_msgs: ActiveMessages,
    pub standby: Standby,
    pub start_time: OffsetDateTime,
    pub metrics: MetricsReader,
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

    pub fn ordr(&self) -> Option<&Ordr> {
        self.clients.ordr.as_deref()
    }

    pub fn psql(&self) -> &Database {
        &self.clients.psql
    }

    #[cfg(feature = "osutracking")]
    pub fn tracking(&self) -> &crate::tracking::OsuTracking {
        &self.data.osu_tracking
    }

    pub fn guild_shards(&self) -> &GuildShards {
        &self.data.guild_shards
    }

    pub fn miss_analyzer_guilds(&self) -> &MissAnalyzerGuilds {
        &self.data.miss_analyzer_guilds
    }

    pub fn has_miss_analyzer(&self, guild: &Id<GuildMarker>) -> bool {
        self.miss_analyzer_guilds().pin().contains(guild)
    }

    #[cfg(feature = "twitch")]
    pub fn online_twitch_streams(&self) -> &crate::tracking::OnlineTwitchStreams {
        &self.data.online_twitch_streams
    }

    pub async fn new(tx: UnboundedSender<(Id<GuildMarker>, u64)>) -> Result<ContextTuple> {
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

            metrics::set_boxed_recorder(Box::new(prefix))
                .wrap_err("Failed to install metrics recorder")?;

            (prometheus_handle, reader)
        };

        let start_time = OffsetDateTime::now_utc();

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

        let cache = Cache::new(&config.redis_host, config.redis_port, config.redis_db_idx)
            .await
            .wrap_err("Failed to create redis cache")?;

        let data = ContextData::new(&psql, &cache, application_id)
            .await
            .wrap_err("Failed to create context data")?;

        let resume_data = cache.defrost().await.wrap_err("Failed to defrost cache")?;

        BotMetrics::init(&cache);

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

        let clients = Clients::new(psql, osu, custom_client, ordr);

        let shards = discord_gateway(config, &http, resume_data)
            .await
            .wrap_err("Failed to create discord gateway shards")?;

        let shard_senders: HashMap<_, _> = shards
            .iter()
            .map(|shard| (shard.id().number(), shard.sender()))
            .collect();

        let shard_senders = RwLock::new(shard_senders);

        #[cfg(feature = "server")]
        let (auth_standby, server_tx) = bathbot_server(config, _prometheus, reader.clone())
            .await
            .wrap_err("Failed to create server")?;

        let ctx = Self {
            cache,
            http,
            clients,
            shard_senders,
            data,
            standby: Standby::new(),
            #[cfg(feature = "server")]
            auth_standby,
            buckets: Buckets::new(),
            member_requests: MemberRequests::new(tx),
            active_msgs: ActiveMessages::new(),
            start_time,
            metrics: reader,
        };

        Ok((
            ctx,
            shards,
            #[cfg(feature = "server")]
            server_tx,
        ))
    }

    /// Acquire an entry for the user in the bucket and optionally return the
    /// cooldown in amount of seconds if acquiring the entry was ratelimitted.
    pub fn check_ratelimit(&self, user_id: Id<UserMarker>, bucket: BucketName) -> Option<i64> {
        let ratelimit = self.buckets.get(bucket).lock().unwrap().take(user_id.get());

        (ratelimit > 0).then_some(ratelimit)
    }

    pub async fn down_resumable(shards: &mut [Shard]) -> HashMap<u64, Session, IntHasher> {
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
                    warn!(shard_id, ?err, "Failed to close shard");

                    future::ready(None)
                }
            })
            .collect()
            .await
    }

    pub async fn reshard(&self, shards: &mut Vec<Shard>) -> Result<()> {
        info!("Resharding...");

        *shards = discord_gateway(BotConfig::get(), &self.http, HashMap::default())
            .await
            .wrap_err("Failed to create new shards for resharding")?;

        let mut unlocked = self.shard_senders.write().unwrap();

        *unlocked = shards
            .iter()
            .map(|shard| (shard.id().number(), shard.sender()))
            .collect();

        info!("Finished resharding");

        Ok(())
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
    ordr: Option<Arc<Ordr>>,
}

impl Clients {
    fn new(psql: Database, osu: Osu, custom: BathbotClient, ordr: Option<Arc<Ordr>>) -> Self {
        Self {
            psql,
            osu,
            custom,
            ordr,
        }
    }
}

struct ContextData {
    application_id: Id<ApplicationMarker>,
    games: Games,
    #[cfg(feature = "matchlive")]
    matchlive: crate::matchlive::MatchLiveChannels,
    #[cfg(feature = "osutracking")]
    osu_tracking: crate::tracking::OsuTracking,
    guild_configs: GuildConfigs,              // read-heavy
    tracked_streams: TrackedStreams,          // read-heavy
    guild_shards: GuildShards,                // necessary to request members for a guild
    miss_analyzer_guilds: MissAnalyzerGuilds, // read-heavy
    osutrack_user_notif_timestamps: OsuTrackUserNotifTimestamps,
    #[cfg(feature = "twitch")]
    online_twitch_streams: crate::tracking::OnlineTwitchStreams,
}

impl ContextData {
    async fn new(
        psql: &Database,
        cache: &Cache,
        application_id: Id<ApplicationMarker>,
    ) -> Result<Self> {
        let (guild_configs_res, tracked_streams_res, guild_shards, miss_analyzer_guilds) = tokio::join!(
            psql.select_guild_configs::<IntHasher>(),
            psql.select_tracked_twitch_streams::<IntHasher>(),
            Self::fetch_guild_shards(cache),
            Self::fetch_miss_analyzer_guilds(cache),
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
            guild_shards,
            #[cfg(feature = "matchlive")]
            matchlive: crate::matchlive::MatchLiveChannels::new(),
            #[cfg(feature = "osutracking")]
            osu_tracking: crate::tracking::OsuTracking::new(
                crate::manager::OsuTrackingManager::new(psql),
            )
            .await
            .wrap_err("Failed to create osu tracking")?,
            miss_analyzer_guilds,
            osutrack_user_notif_timestamps: OsuTrackUserNotifTimestamps::default(),
            #[cfg(feature = "twitch")]
            online_twitch_streams: crate::tracking::OnlineTwitchStreams::default(),
        })
    }

    async fn fetch_guild_shards(cache: &Cache) -> GuildShards {
        let fetch_fut = cache.fetch::<_, Vec<(With<Id<GuildMarker>, IdRkyv>, u64)>>("guild_shards");

        match fetch_fut.await {
            Ok(Ok(guild_shards)) => guild_shards.iter().collect(),
            Ok(Err(_)) => GuildShards::default(),
            Err(err) => {
                warn!(?err, "Failed to fetch guild shards, creating default...");

                GuildShards::default()
            }
        }
    }

    async fn fetch_miss_analyzer_guilds(cache: &Cache) -> MissAnalyzerGuilds {
        let fetch_fut =
            cache.fetch::<_, Vec<With<Id<GuildMarker>, IdRkyv>>>("miss_analyzer_guilds");

        match fetch_fut.await {
            Ok(Ok(miss_analyzer_guilds)) => miss_analyzer_guilds.iter().collect(),
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

type BgGames = TokioRwLockMap<Id<ChannelMarker>, BackgroundGame, IntHasher>;

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
    resume_data: HashMap<u64, Session, IntHasher>,
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
        // .large_threshold(250) // requires presence intent to have an effect
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
