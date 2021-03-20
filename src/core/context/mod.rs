mod impls;

use super::BotStats;

use crate::{
    bg_game::GameWrapper,
    core::{
        buckets::{buckets, Buckets},
        Cache,
    },
    database::{Database, GuildConfig},
    BotResult, CustomClient, OsuTracking, Twitch,
};

use darkredis::ConnectionPool;
use dashmap::{DashMap, DashSet};
use rosu_v2::Osu;
use std::{collections::HashSet, sync::Arc};
use tokio::sync::Mutex;
use twilight_gateway::Cluster;
use twilight_http::Client as HttpClient;
use twilight_model::{
    gateway::{
        payload::UpdateStatus,
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
    pub backend: BackendData,
    pub clients: Clients,
    // private to avoid deadlocks by messing up references
    data: ContextData,
}

pub struct Clients {
    pub psql: Database,
    pub redis: ConnectionPool,
    pub osu: Osu,
    pub custom: CustomClient,
    pub twitch: Twitch,
}

pub struct BackendData {
    pub cluster: Cluster,
    pub total_shards: u64,
    pub shards_per_cluster: u64,
}

pub struct ContextData {
    pub guilds: DashMap<GuildId, GuildConfig>,
    // Mapping twitch user ids to vec of discord channel ids
    pub tracked_streams: DashMap<u64, Vec<u64>>,
    // Mapping (channel id, message id) to role id
    pub role_assigns: DashMap<(u64, u64), u64>,
    pub discord_links: DashMap<u64, String>,
    pub bg_games: DashMap<ChannelId, GameWrapper>,
    pub osu_tracking: OsuTracking,
    pub msgs_to_process: DashSet<MessageId>,
    pub map_garbage_collection: Mutex<HashSet<u32>>,
}

impl Context {
    #[cold]
    #[inline]
    pub async fn new(
        cache: Cache,
        stats: Arc<BotStats>,
        http: HttpClient,
        clients: Clients,
        backend: BackendData,
        data: ContextData,
    ) -> Self {
        Context {
            cache,
            stats,
            http,
            standby: Standby::new(),
            clients,
            backend,
            data,
            buckets: buckets(),
        }
    }

    pub async fn set_cluster_activity(
        &self,
        status: Status,
        activity_type: ActivityType,
        message: String,
    ) -> BotResult<()> {
        for shard_id in 0..self.backend.total_shards {
            debug!("Setting activity for shard {}", shard_id);

            self.set_shard_activity(shard_id, status, activity_type, message.clone())
                .await?;
        }

        Ok(())
    }

    pub async fn set_shard_activity(
        &self,
        shard_id: u64,
        status: Status,
        activity_type: ActivityType,
        message: impl Into<String>,
    ) -> BotResult<()> {
        self.backend
            .cluster
            .command(
                shard_id,
                &UpdateStatus::new(
                    Some(vec![generate_activity(activity_type, message.into())]),
                    false,
                    None,
                    status,
                ),
            )
            .await?;

        Ok(())
    }
}

#[inline]
pub fn generate_activity(activity_type: ActivityType, message: String) -> Activity {
    Activity {
        assets: None,
        application_id: None,
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
