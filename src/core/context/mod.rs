mod impls;

pub use impls::{MatchLiveChannels, MatchTrackResult};

use crate::{
    bg_game::GameWrapper,
    core::{
        buckets::{buckets, Buckets},
        BotStats, Cache,
    },
    database::{Database, GuildConfig},
    util::CountryCode,
    BotResult, CustomClient, OsuTracking, Twitch,
};

use dashmap::{DashMap, DashSet};
use deadpool_redis::Pool as RedisPool;
use hashbrown::HashSet;
use rosu_v2::Osu;
use std::sync::Arc;
use tokio::sync::Mutex;
use twilight_gateway::Cluster;
use twilight_http::Client as HttpClient;
use twilight_model::{
    gateway::{
        payload::UpdatePresence,
        presence::{Activity, ActivityType, Status},
    },
    id::{ChannelId, GuildId, MessageId},
};
use twilight_standby::Standby;

pub struct Context {
    pub cache: Cache,
    pub stats: Arc<BotStats>,
    pub http: HttpClient,
    pub standby: Standby,
    pub buckets: Buckets,
    pub cluster: Cluster,
    pub clients: Clients,
    // private to avoid deadlocks by messing up references
    data: ContextData,
}

pub struct Clients {
    pub psql: Database,
    pub redis: RedisPool,
    pub osu: Osu,
    pub custom: CustomClient,
    pub twitch: Twitch,
}

pub struct ContextData {
    // ! CAREFUL: When entries are added or modified
    // ! don't forget to update the DB entry aswell
    pub guilds: DashMap<GuildId, GuildConfig>,
    // Mapping twitch user ids to vec of discord channel ids
    pub tracked_streams: DashMap<u64, Vec<u64>>,
    // Mapping (channel id, message id) to role id
    pub role_assigns: DashMap<(u64, u64), u64>,
    pub bg_games: DashMap<ChannelId, GameWrapper>,
    pub osu_tracking: OsuTracking,
    pub msgs_to_process: DashSet<MessageId>,
    pub map_garbage_collection: Mutex<HashSet<u32>>,
    pub match_live: MatchLiveChannels,
    pub snipe_countries: DashMap<CountryCode, String>,
}

impl Context {
    #[cold]
    pub async fn new(
        cache: Cache,
        stats: Arc<BotStats>,
        http: HttpClient,
        clients: Clients,
        cluster: Cluster,
        data: ContextData,
    ) -> Self {
        Context {
            cache,
            stats,
            http,
            standby: Standby::new(),
            clients,
            cluster,
            data,
            buckets: buckets(),
        }
    }

    pub async fn set_cluster_activity<M>(
        &self,
        status: Status,
        activity_type: ActivityType,
        message: M,
    ) -> BotResult<()>
    where
        M: Into<String> + Clone,
    {
        let [_, total_shards] = self.cluster.config().shard_config().shard();

        for shard_id in 1..total_shards {
            debug!("Setting activity for shard {}", shard_id);

            self.set_shard_activity(shard_id, status, activity_type, message.clone())
                .await?;
        }

        debug!("Setting activity for shard 0");

        self.set_shard_activity(0, status, activity_type, message)
            .await?;

        Ok(())
    }

    pub async fn set_shard_activity(
        &self,
        shard_id: u64,
        status: Status,
        activity_type: ActivityType,
        message: impl Into<String>,
    ) -> BotResult<()> {
        let activities = vec![generate_activity(activity_type, message.into())];
        let status = UpdatePresence::new(activities, false, None, status).unwrap();
        self.cluster.command(shard_id, &status).await?;

        Ok(())
    }

    pub fn osu(&self) -> &Osu {
        &self.clients.osu
    }

    pub fn psql(&self) -> &Database {
        &self.clients.psql
    }
}

pub fn generate_activity(activity_type: ActivityType, message: String) -> Activity {
    Activity {
        assets: None,
        application_id: None,
        buttons: Vec::new(),
        created_at: None,
        details: None,
        flags: None,
        id: None,
        instance: None,
        kind: activity_type,
        name: message,
        emoji: None,
        party: None,
        secrets: None,
        state: None,
        timestamps: None,
        url: None,
    }
}
