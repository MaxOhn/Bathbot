mod impls;

use super::ShardState;

use crate::{
    bg_game::GameWrapper,
    core::{
        buckets::{Bucket, Ratelimit},
        stored_values::StoredValues,
        Cache,
    },
    database::{Database, GuildConfig},
    BotResult, CustomClient, Twitch,
};

use darkredis::ConnectionPool;
use dashmap::DashMap;
use rosu::Osu;
use std::collections::HashMap;
use tokio::sync::Mutex;
use twilight::gateway::Cluster;
use twilight::http::Client as HttpClient;
use twilight::model::{
    gateway::{
        payload::UpdateStatus,
        presence::{Activity, ActivityType, Status},
    },
    id::{ChannelId, GuildId},
};
use twilight::standby::Standby;

type Buckets = DashMap<&'static str, Mutex<Bucket>>;

pub struct Context {
    pub cache: Cache,
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
    pub shard_states: DashMap<u64, ShardState>,
    pub total_shards: u64,
    pub shards_per_cluster: u64,
}

pub struct ContextData {
    pub guilds: DashMap<GuildId, GuildConfig>,
    pub stored_values: StoredValues,
    pub perf_calc_mutex: Mutex<()>,
    // Mapping twitch user ids to vec of discord channel ids
    pub tracked_streams: DashMap<u64, Vec<u64>>,
    // Mapping (channel id, message id) to role id
    pub role_assigns: DashMap<(u64, u64), u64>,
    pub discord_links: DashMap<u64, String>,
    pub bg_games: DashMap<ChannelId, GameWrapper>,
}

impl Context {
    pub async fn new(
        cache: Cache,
        http: HttpClient,
        clients: Clients,
        backend: BackendData,
        data: ContextData,
    ) -> Self {
        cache
            .stats
            .shard_counts
            .pending
            .set(backend.shards_per_cluster as i64);
        Context {
            cache,
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
        for shard_id in 0..self.backend.shards_per_cluster {
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
                    false,
                    generate_activity(activity_type, message.into()),
                    None,
                    status,
                ),
            )
            .await?;
        Ok(())
    }
}

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

fn buckets() -> Buckets {
    let buckets = DashMap::new();
    insert_bucket(&buckets, "songs", 20, 0, 1);
    insert_bucket(&buckets, "bg_start", 0, 30, 4);
    insert_bucket(&buckets, "bg_bigger", 0, 11, 3);
    insert_bucket(&buckets, "bg_hint", 0, 8, 3);
    buckets
}

fn insert_bucket(buckets: &Buckets, name: &'static str, delay: i64, time_span: i64, limit: i32) {
    buckets.insert(
        name,
        Mutex::new(Bucket {
            ratelimit: Ratelimit {
                delay,
                limit: Some((time_span, limit)),
            },
            users: HashMap::new(),
        }),
    );
}
