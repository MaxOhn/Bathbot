use super::{cold_current_user::ColdStorageCurrentUser, Cache};
use crate::{BotResult, Error};

use darkredis::ConnectionPool;
use dashmap::DashMap;
use futures::future;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Instant};
use twilight_cache_inmemory::CachedGuild;
use twilight_gateway::shard::ResumeSession;
use twilight_model::{id::GuildId, user::CurrentUser};

const STORE_DURATION: u32 = 180; // seconds

#[derive(Deserialize, Serialize, Debug)]
pub struct ColdRebootData {
    pub resume_data: HashMap<u64, (String, u64)>,
    pub shard_count: u64,
    pub total_shards: u64,
    pub guild_chunks: usize,
}

impl Cache {
    // ###################
    // ## Defrost cache ##
    // ###################

    pub async fn restore_cold_resume(
        redis: &ConnectionPool,
        reboot_data: ColdRebootData,
    ) -> Result<(DashMap<GuildId, Arc<CachedGuild>>, CurrentUser), (&'static str, Error)> {
        // --- Guilds ---
        let guilds = DashMap::new();
        let guild_defrosters: Vec<_> = (0..reboot_data.guild_chunks)
            .map(|i| defrost_guilds(redis, i, &guilds))
            .collect();
        future::try_join_all(guild_defrosters)
            .await
            .map_err(|e| ("guilds", e))?;
        // --- CurrentUser ---
        let user = defrost_current_user(redis)
            .await
            .map_err(|e| ("current_user", e))?;
        debug!("Defrosting {} guilds completed", guilds.len(),);
        Ok((guilds, user))
    }

    // ##################
    // ## Freeze cache ##
    // ##################

    pub async fn prepare_cold_resume(
        &self,
        redis: &ConnectionPool,
        resume_data: HashMap<u64, ResumeSession>,
        total_shards: u64,
        shards_per_cluster: u64,
    ) -> BotResult<()> {
        for i in 1..5 {
            println!("i: {}", i);
            tokio::time::delay_for(tokio::time::Duration::from_secs(1)).await;
        }
        let start = Instant::now();
        // --- Guilds ---
        let cache_guilds = self.0.guilds();
        let guild_chunks = cache_guilds.len() / 100_000 + 1;
        let mut guild_work_orders = vec![Vec::with_capacity(1000); guild_chunks];
        for (i, guard) in cache_guilds.iter().enumerate() {
            guild_work_orders[i % guild_chunks].push(*guard.key());
        }
        debug!("Freezing {} guilds", cache_guilds.len());
        let guild_tasks: Vec<_> = guild_work_orders
            .into_iter()
            .enumerate()
            .map(|(i, order)| self._prepare_cold_resume_guild(redis, order, i))
            .collect();
        future::join_all(guild_tasks).await;

        // --- CurrentUser ---
        debug!("Freezing current user");
        self._prepare_cold_resume_current_user(redis).await;

        // ------

        // Prepare resume data
        let map: HashMap<_, _> = resume_data
            .into_iter()
            .map(|(shard_id, info)| (shard_id, (info.session_id, info.sequence)))
            .collect();
        let data = ColdRebootData {
            resume_data: map,
            total_shards,
            guild_chunks,
            shard_count: shards_per_cluster,
        };
        let mut connection = redis.get().await;
        println!("setting data...");
        connection
            .set_and_expire_seconds(
                "cb_cluster_data",
                &serde_json::to_value(data).unwrap().to_string().into_bytes(),
                STORE_DURATION,
            )
            .await?;
        let end = Instant::now();
        info!(
            "Cold resume preparations completed in {}ms",
            (end - start).as_millis()
        );
        Ok(())
    }

    pub async fn _prepare_cold_resume_guild(
        &self,
        redis: &ConnectionPool,
        orders: Vec<GuildId>,
        index: usize,
    ) {
        debug!(
            "Guild dumper {} started freezing {} guilds",
            index,
            orders.len()
        );
        println!("waiting for connection");
        let mut connection = redis.get().await;
        println!("got connection");
        let to_dump: Vec<_> = orders
            .into_iter()
            .filter_map(|key| self.0.guilds().remove(&key))
            .map(|(_, g)| g)
            .collect();
        let serialized = serde_json::to_string(&to_dump).unwrap();
        println!("setting...");
        let dump_task = connection
            .set_and_expire_seconds(
                format!("cb_cluster_guild_chunk_{}", index),
                serialized,
                STORE_DURATION,
            )
            .await;
        println!("done setting");
        if let Err(why) = dump_task {
            debug!(
                "Error while setting redis' `cb_cluster_guild_chunk_{}`: {}",
                index, why
            );
        }
    }

    pub async fn _prepare_cold_resume_current_user(&self, redis: &ConnectionPool) {
        if let Some(user) = self.0.current_user() {
            let mut connection = redis.get().await;
            let user = ColdStorageCurrentUser {
                avatar: user.avatar.to_owned(),
                discriminator: user.discriminator.to_owned(),
                flags: user.flags,
                id: user.id,
                locale: user.locale.to_owned(),
                mfa_enabled: user.mfa_enabled,
                name: user.name.to_owned(),
                premium_type: user.premium_type,
                public_flags: user.public_flags,
                verified: user.verified,
            };
            let serialized = serde_json::to_string(&user).unwrap();
            let dump_task = connection
                .set_and_expire_seconds("cb_cluster_current_user", serialized, STORE_DURATION)
                .await;
            if let Err(why) = dump_task {
                debug!(
                    "Error while setting redis' `cb_cluster_current_user`: {}",
                    why
                );
            }
        }
    }
}

async fn defrost_guilds(
    redis: &ConnectionPool,
    index: usize,
    guilds: &DashMap<GuildId, Arc<CachedGuild>>,
) -> BotResult<()> {
    let key = format!("cb_cluster_guild_chunk_{}", index);
    let mut connection = redis.get().await;
    let data = connection.get(&key).await?.unwrap();
    let guilds_cached: Vec<CachedGuild> = serde_json::from_slice(&data)?;
    connection.del(key).await?;
    debug!(
        "Worker {} found {} guilds to defrost",
        index,
        guilds_cached.len()
    );
    for guild in guilds_cached {
        guilds.insert(guild.id, Arc::new(guild));
    }
    Ok(())
}

async fn defrost_current_user(redis: &ConnectionPool) -> BotResult<CurrentUser> {
    let key = "cb_cluster_current_user";
    let mut connection = redis.get().await;
    let data = connection.get(key).await?.unwrap();
    let user: ColdStorageCurrentUser = serde_json::from_slice(&data)?;
    connection.del(key).await?;
    debug!("Worker found current user to defrost");
    Ok(user.into())
}
