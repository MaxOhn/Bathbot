use super::ShardState;

use crate::{
    core::{
        stored_values::{StoredValues, Values},
        Cache, ColdRebootData,
    },
    database::{Database, GuildConfig},
    BotResult, CustomClient,
};

use darkredis::ConnectionPool;
use dashmap::DashMap;
use rosu::{
    models::{GameMode, GameMods},
    Osu,
};
use std::{collections::HashMap, time::Instant};
use tokio::sync::Mutex;
use twilight::gateway::Cluster;
use twilight::http::Client as HttpClient;
use twilight::model::{
    channel::Message,
    gateway::{
        payload::UpdateStatus,
        presence::{Activity, ActivityType, Status},
    },
    id::GuildId,
};
use twilight::standby::Standby;

pub struct Context {
    pub cache: Cache,
    pub http: HttpClient,
    pub standby: Standby,
    pub guilds: DashMap<GuildId, GuildConfig>,
    pub stored_values: StoredValues,
    pub perf_calc_mutex: Mutex<()>,
    // Mapping twitch user ids to vec of discord channel ids
    pub tracked_streams: DashMap<u64, Vec<u64>>,
    // Mapping (channel id, message id) to role id
    pub role_assigns: DashMap<(u64, u64), u64>,
    pub backend: BackendData,
    pub clients: Clients,
}

pub struct Clients {
    pub psql: Database,
    pub redis: ConnectionPool,
    pub osu: Osu,
    pub custom: CustomClient,
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
        http: HttpClient,
        clients: Clients,
        backend: BackendData,
        stored_values: StoredValues,
        tracked_streams: DashMap<u64, Vec<u64>>,
        role_assigns: DashMap<(u64, u64), u64>,
        guilds: DashMap<GuildId, GuildConfig>,
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
            guilds,
            clients,
            backend,
            stored_values,
            tracked_streams,
            role_assigns,
            perf_calc_mutex: Mutex::new(()),
        }
    }

    /// Returns if a message was sent by us
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
        let (guild_chunks, user_chunks) = self.cache.prepare_cold_resume(&self.clients.redis).await;

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
        // TODO: remove 0 from redis name
        connection
            .set_and_expire_seconds(
                "cb_cluster_data_0",
                &serde_json::to_value(data).unwrap().to_string().into_bytes(),
                180,
            )
            .await
            .unwrap();
        let end = Instant::now();
        debug!(
            "Cold resume preparations completed in {}ms",
            (end - start).as_millis()
        );
        Ok(())
    }

    pub async fn store_values(&self) -> BotResult<()> {
        let start = Instant::now();
        let mania_pp = &self.stored_values.mania_pp;
        let mania_stars = &self.stored_values.mania_stars;
        let ctb_pp = &self.stored_values.ctb_pp;
        let ctb_stars = &self.stored_values.ctb_stars;
        let psql = &self.clients.psql;
        let (mania_pp, mania_stars, ctb_pp, ctb_stars) = tokio::try_join!(
            psql.insert_mania_pp(mania_pp),
            psql.insert_mania_stars(mania_stars),
            psql.insert_ctb_pp(ctb_pp),
            psql.insert_ctb_stars(ctb_stars),
        )?;
        let end = Instant::now();
        debug!(
            "Stored {} pp and {} star values in {}ms",
            mania_pp + ctb_pp,
            mania_stars + ctb_stars,
            (end - start).as_millis()
        );
        Ok(())
    }

    pub fn pp(&self, mode: GameMode) -> &Values {
        match mode {
            GameMode::MNA => &self.stored_values.mania_pp,
            GameMode::CTB => &self.stored_values.ctb_pp,
            _ => unreachable!(),
        }
    }

    pub fn stars(&self, mode: GameMode) -> &Values {
        match mode {
            GameMode::MNA => &self.stored_values.mania_stars,
            GameMode::CTB => &self.stored_values.ctb_stars,
            _ => unreachable!(),
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
