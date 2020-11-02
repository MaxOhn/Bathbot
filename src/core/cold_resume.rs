use crate::{
    core::cache::{Cache, CachedGuild, CachedUser, ColdStorageGuild},
    BotResult, Error,
};

use darkredis::ConnectionPool;
use futures::future;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use twilight_model::id::{GuildId, UserId};

#[derive(Deserialize, Serialize, Debug)]
pub struct ColdRebootData {
    pub resume_data: HashMap<u64, (String, u64)>,
    pub shard_count: u64,
    pub total_shards: u64,
    pub guild_chunks: usize,
    pub user_chunks: usize,
}

impl Cache {
    // ##################
    // ## Freeze cache ##
    // ##################

    pub async fn prepare_cold_resume(&self, redis: &ConnectionPool) -> (usize, usize) {
        // Clear global caches so arcs can be cleaned up
        self.guild_channels.clear();
        // We do not want to drag along DM channels, we get guild creates for them when they send a message anyways
        self.private_channels.clear();
        // Collect their work first before they start sabotaging each other again >.>
        let mut work_orders: Vec<Vec<GuildId>> = vec![];
        let mut count = 0;
        let mut list = vec![];
        for guard in self.guilds.iter() {
            count +=
                guard.members.len() + guard.channels.len() + guard.emoji.len() + guard.roles.len();
            list.push(*guard.key());
            if count > 100_000 {
                work_orders.push(list);
                list = vec![];
                count = 0;
            }
        }
        if !list.is_empty() {
            work_orders.push(list)
        }
        debug!("Freezing {} guilds", self.stats.guild_counts.loaded.get());
        let tasks: Vec<_> = work_orders
            .into_iter()
            .enumerate()
            .map(|(i, order)| self._prepare_cold_resume_guild(redis, order, i))
            .collect();
        let guild_chunks = tasks.len();
        future::join_all(tasks).await;
        count = 0;
        let user_chunks = (self.users.len() / 100_000 + 1) as usize;
        let mut user_work_orders: Vec<Vec<UserId>> = vec![Vec::with_capacity(50_000); user_chunks];
        for guard in self.users.iter() {
            user_work_orders[count % user_chunks].push(*guard.key());
            count += 1;
        }
        debug!("Freezing {} users", self.users.len());
        let user_tasks: Vec<_> = user_work_orders
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| self._prepare_cold_resume_user(redis, chunk, i))
            .collect();
        future::join_all(user_tasks).await;
        self.users.clear();
        (guild_chunks, user_chunks)
    }

    async fn _prepare_cold_resume_guild(
        &self,
        redis: &ConnectionPool,
        orders: Vec<GuildId>,
        index: usize,
    ) -> Result<(), Error> {
        debug!(
            "Guild dumper {} started freezing {} guilds",
            index,
            orders.len()
        );
        let mut connection = redis.get().await;
        let to_dump: Vec<_> = orders
            .into_par_iter()
            .filter_map(|key| self.guilds.remove(&key))
            .map(|(_, g)| g)
            .map(ColdStorageGuild::from)
            .collect();
        let serialized = serde_json::to_string(&to_dump).unwrap();
        let dump_task = connection
            .set_and_expire_seconds(format!("cb_cluster_guild_chunk_{}", index), serialized, 180)
            .await;
        if let Err(why) = dump_task {
            debug!(
                "Error while setting redis' `cb_cluster_guild_chunk_{}`: {}",
                index, why
            );
        }
        Ok(())
    }

    async fn _prepare_cold_resume_user(
        &self,
        redis: &ConnectionPool,
        chunk: Vec<UserId>,
        index: usize,
    ) -> Result<(), Error> {
        debug!("Worker {} freezing {} users", index, chunk.len());
        let mut connection = redis.get().await;
        let users: Vec<_> = chunk
            .into_par_iter()
            .filter_map(|key| self.users.remove(&key))
            .map(|(_, user)| CachedUser {
                id: user.id,
                username: user.username.clone(),
                discriminator: user.discriminator,
                avatar: user.avatar.clone(),
                bot_user: user.bot_user,
                system_user: user.system_user,
                public_flags: user.public_flags,
                mutual_servers: AtomicU64::new(0),
            })
            .collect();
        let serialized = serde_json::to_string(&users).unwrap();
        let worker_task = connection
            .set_and_expire_seconds(format!("cb_cluster_user_chunk_{}", index), serialized, 180)
            .await;
        if let Err(why) = worker_task {
            debug!(
                "Error while setting redis' `cb_cluster_user_chunk_{}`: {}",
                index, why
            );
        }
        Ok(())
    }

    // ###################
    // ## Defrost cache ##
    // ###################

    async fn defrost_users(&self, redis: &ConnectionPool, index: usize) -> BotResult<()> {
        let key = format!("cb_cluster_user_chunk_{}", index);
        let mut connection = redis.get().await;
        let mut users: Vec<CachedUser> = serde_json::from_str(
            &String::from_utf8(connection.get(&key).await?.unwrap()).unwrap(),
        )?;
        connection.del(key).await?;
        debug!("Worker {} found {} users to defrost", index, users.len());
        for user in users.drain(..) {
            self.users.insert(user.id, Arc::new(user));
            self.stats.user_counts.unique.inc();
        }
        Ok(())
    }

    async fn defrost_guilds(&self, redis: &ConnectionPool, index: usize) -> BotResult<()> {
        let key = format!("cb_cluster_guild_chunk_{}", index);
        let mut connection = redis.get().await;
        let mut guilds: Vec<ColdStorageGuild> = serde_json::from_str(
            &String::from_utf8(connection.get(&key).await?.unwrap()).unwrap(),
        )?;
        connection.del(key).await?;
        debug!("Worker {} found {} guilds to defrost", index, guilds.len());
        for cold_guild in guilds.drain(..) {
            let guild = CachedGuild::defrost(&self, cold_guild);
            for channel in &guild.channels {
                self.guild_channels
                    .insert(channel.get_id(), channel.value().clone());
            }
            self.stats.channel_count.add(guild.channels.len() as i64);
            for emoji in &guild.emoji {
                self.emoji.insert(emoji.id, Arc::clone(emoji));
            }
            self.stats.user_counts.total.add(guild.members.len() as i64);
            self.guilds.insert(guild.id, Arc::new(guild));
            self.stats.guild_counts.loaded.inc();
        }
        Ok(())
    }

    pub async fn restore_cold_resume(
        &self,
        redis: &ConnectionPool,
        guild_chunks: usize,
        user_chunks: usize,
    ) -> BotResult<()> {
        let user_defrosters: Vec<_> = (0..user_chunks)
            .map(|i| self.defrost_users(redis, i))
            .collect();
        let results = future::join_all(user_defrosters).await;
        if let Some(Err(why)) = results.into_iter().find(|r| r.is_err()) {
            return Err(Error::CacheDefrost("users", Box::new(why)));
        }
        let guild_defrosters: Vec<_> = (0..guild_chunks)
            .map(|i| self.defrost_guilds(redis, i))
            .collect();
        let results = future::join_all(guild_defrosters).await;
        if let Some(Err(why)) = results.into_iter().find(|r| r.is_err()) {
            return Err(Error::CacheDefrost("guilds", Box::new(why)));
        }
        self.filling.store(false, Ordering::SeqCst);
        info!(
            "Cache defrosting complete, now holding {} users ({} unique) from {} guilds ({} channels)",
            self.stats.user_counts.total.get(),
            self.stats.user_counts.unique.get(),
            self.stats.guild_counts.loaded.get(),
            self.stats.channel_count.get(),
        );
        Ok(())
    }
}
