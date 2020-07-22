use crate::{
    core::cache::{Cache, CachedGuild, CachedUser, ColdStorageGuild},
    BotResult, Error,
};

use darkredis::ConnectionPool;
use futures::future;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use twilight::model::id::{GuildId, UserId};

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
        let mut tasks = vec![];
        let mut user_tasks = vec![];
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
        for (i, order) in work_orders.iter().enumerate() {
            tasks.push(self._prepare_cold_resume_guild(redis, order.clone(), i));
        }
        let guild_chunks = tasks.len();
        future::join_all(tasks).await;
        count = 0;
        let user_chunks = (self.users.len() / 100_000 + 1) as usize;
        let mut user_work_orders: Vec<Vec<UserId>> = vec![vec![]; user_chunks];
        for guard in self.users.iter() {
            user_work_orders[count % user_chunks].push(*guard.key());
            count += 1;
        }
        debug!("Freezing {} users", self.users.len());
        for (i, chunk) in user_work_orders.iter().enumerate().take(user_chunks) {
            user_tasks.push(self._prepare_cold_resume_user(redis, chunk.clone(), i));
        }
        future::join_all(user_tasks).await;
        self.users.clear();
        (guild_chunks, user_chunks)
    }

    async fn _prepare_cold_resume_guild(
        &self,
        redis: &ConnectionPool,
        todo: Vec<GuildId>,
        index: usize,
    ) -> Result<(), Error> {
        debug!(
            "Guild dumper {} started freezing {} guilds",
            index,
            todo.len()
        );
        let mut connection = redis.get().await;
        let mut to_dump = Vec::with_capacity(todo.len());
        for key in todo {
            let g = self.guilds.remove_take(&key).unwrap();
            to_dump.push(ColdStorageGuild::from(g));
        }
        let serialized = serde_json::to_string(&to_dump).unwrap();
        connection
            .set_and_expire_seconds(
                format!("cb_cluster_{}_guild_chunk_{}", self.cluster_id, index),
                serialized,
                300,
            )
            .await?;
        Ok(())
    }

    async fn _prepare_cold_resume_user(
        &self,
        redis: &ConnectionPool,
        todo: Vec<UserId>,
        index: usize,
    ) -> Result<(), Error> {
        debug!("Worker {} freezing {} users", index, todo.len());
        let mut connection = redis.get().await;
        let mut chunk = Vec::with_capacity(todo.len());
        for key in todo {
            let user = self.users.remove_take(&key).unwrap();
            chunk.push(CachedUser {
                id: user.id,
                username: user.username.clone(),
                discriminator: user.discriminator.clone(),
                avatar: user.avatar.clone(),
                bot_user: user.bot_user,
                system_user: user.system_user,
                public_flags: user.public_flags,
                mutual_servers: AtomicU64::new(0),
            });
        }
        let serialized = serde_json::to_string(&chunk).unwrap();
        connection
            .set_and_expire_seconds(
                format!("cb_cluster_{}_user_chunk_{}", self.cluster_id, index),
                serialized,
                300,
            )
            .await?;
        Ok(())
    }

    // ###################
    // ## Defrost cache ##
    // ###################

    async fn defrost_users(&self, redis: &ConnectionPool, index: usize) -> BotResult<()> {
        let key = format!("cb_cluster_{}_user_chunk_{}", self.cluster_id, index);
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
        let key = format!("cb_cluster_{}_guild_chunk_{}", self.cluster_id, index);
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
                self.emoji.insert(emoji.id, emoji.clone());
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
        let mut user_defrosters = Vec::with_capacity(user_chunks);
        for i in 0..user_chunks {
            user_defrosters.push(self.defrost_users(redis, i));
        }
        for result in future::join_all(user_defrosters).await {
            if let Err(why) = result {
                return Err(Error::CacheDefrost("users", Box::new(why)));
            }
        }
        let mut guild_defrosters = Vec::with_capacity(guild_chunks);
        for i in 0..guild_chunks {
            guild_defrosters.push(self.defrost_guilds(redis, i));
        }
        for result in future::join_all(guild_defrosters).await {
            if let Err(why) = result {
                return Err(Error::CacheDefrost("guilds", Box::new(why)));
            }
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
