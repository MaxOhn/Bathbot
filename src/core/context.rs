use super::{BotStats, ShardState};

use crate::{
    core::{Cache, ColdRebootData},
    database::{Database, GuildConfig},
    BotResult,
};

use darkredis::ConnectionPool;
use dashmap::DashMap;
use std::{collections::HashMap, sync::Arc, time::Instant};
use twilight::{
    gateway::Cluster,
    http::Client as HttpClient,
    model::{
        channel::Message,
        gateway::{
            payload::UpdateStatus,
            presence::{Activity, ActivityType, Status},
        },
        id::GuildId,
    },
    standby::Standby,
};

pub struct Context {
    pub cache: Cache,
    pub http: HttpClient,
    pub standby: Standby,
    pub stats: Arc<BotStats>,
    pub guilds: DashMap<GuildId, GuildConfig>,
    pub backend: BackendData,
    pub clients: Clients,
}

pub struct Clients {
    pub psql: Database,
    pub redis: ConnectionPool,
    // pub osu: Osu,
    // pub custom: CustomScraper,
}

pub struct BackendData {
    pub cluster: Cluster,
    pub shard_states: DashMap<u64, ShardState>,
    pub total_shards: u64,
    pub shards_per_cluster: u64,
}

impl Context {
    pub async fn new(
        cache: Cache,
        cluster: Cluster,
        http: HttpClient,
        database: Database,
        redis: ConnectionPool,
        stats: Arc<BotStats>,
        total_shards: u64,
        shards_per_cluster: u64,
    ) -> Self {
        let shard_states = DashMap::with_capacity(shards_per_cluster as usize);
        for i in 0..shards_per_cluster {
            shard_states.insert(i, ShardState::PendingCreation);
        }
        stats.shard_counts.pending.set(shards_per_cluster as i64);
        let clients = Clients {
            psql: database,
            redis,
        };
        let backend = BackendData {
            cluster,
            shard_states,
            total_shards,
            shards_per_cluster,
        };
        Context {
            cache,
            http,
            standby: Standby::new(),
            stats,
            guilds: DashMap::new(),
            clients,
            backend,
        }
    }

    /// Returns if a message was sent by us.
    pub fn is_own(&self, other: &Message) -> bool {
        self.cache.bot_user.id == other.author.id
    }

    pub async fn initiate_cold_resume(&self) -> BotResult<()> {
        // Preparing for update rollout, set status to atleast give some indication to users
        info!("Preparing for cold resume");
        self.set_cluster_activity(
            Status::Idle,
            ActivityType::Watching,
            String::from("update deployment, replies might be delayed"),
        )
        .await?;
        let start = Instant::now();
        let mut connection = self.clients.redis.get().await;

        //kill the shards and get their resume info
        //DANGER: WE WILL NOT BE GETTING EVENTS FROM THIS POINT ONWARDS, REBOOT REQUIRED

        let resume_data = self.backend.cluster.down_resumable().await;
        info!("Resume data acquired");
        let (guild_chunks, user_chunks) = self.cache.prepare_cold_resume(&self.clients.redis).await;
        println!(
            "guild chunks: {} ~  user chunks: {}",
            guild_chunks, user_chunks
        );

        // Prepare resume data
        let mut map = HashMap::new();
        for (shard_id, data) in resume_data {
            if let Some(info) = data {
                map.insert(shard_id, (info.session_id, info.sequence));
            }
        }
        let data = ColdRebootData {
            resume_data: map,
            total_shards: self.backend.total_shards,
            guild_chunks,
            shard_count: self.backend.shards_per_cluster,
            user_chunks,
        };
        println!("Setting redis data...");
        connection
            .set_and_expire_seconds(
                "cb_cluster_data_0",
                &serde_json::to_value(data).unwrap().to_string().into_bytes(),
                180,
            )
            .await
            .unwrap();
        let end = Instant::now();
        println!(
            "Cold resume preparations completed in {}ms",
            (end - start).as_millis()
        );
        Ok(())
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
        message: String,
    ) -> BotResult<()> {
        self.backend
            .cluster
            .command(
                shard_id,
                &UpdateStatus::new(
                    false,
                    generate_activity(activity_type, message),
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
