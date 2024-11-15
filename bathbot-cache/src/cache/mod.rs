#![allow(dependency_on_unit_never_type_fallback)] // TODO: remove

use std::fmt::Display;

use bb8_redis::{
    bb8::{Pool, PooledConnection},
    RedisConnectionManager,
};
use eyre::{Result, WrapErr};
use tracing::error;
use twilight_gateway::Event;
use twilight_model::application::interaction::InteractionData;

use crate::model::{CacheChange, CacheStats, CacheStatsInternal};

mod cold_resume;
mod delete;
mod fetch;
mod store;

pub struct Cache {
    redis: Pool<RedisConnectionManager>,
    stats: CacheStatsInternal,
}

impl Cache {
    pub async fn new(host: impl Display, port: u16, db_idx: u8) -> Result<Self> {
        let redis_uri = format!("redis://{host}:{port}/{db_idx}");

        let redis_manager =
            RedisConnectionManager::new(redis_uri).wrap_err("Failed to create redis manager")?;

        let redis = Pool::builder()
            .max_size(16)
            .build(redis_manager)
            .await
            .wrap_err("Failed to create redis pool")?;

        let stats = CacheStatsInternal::new(&redis)
            .await
            .wrap_err("Failed to create cache stats")?;

        Ok(Self { redis, stats })
    }

    pub async fn update(&self, event: &Event) -> Option<CacheChange> {
        async fn update(cache: &Cache, event: &Event) -> Result<Option<CacheChange>> {
            let change = match event {
                Event::ChannelCreate(e) => cache.cache_channel(e).await?,
                Event::ChannelDelete(e) => cache.delete_channel(e.guild_id, e.id).await?,
                Event::ChannelUpdate(e) => cache.cache_channel(e).await?,
                Event::GuildCreate(e) => cache.cache_guild(e).await?,
                Event::GuildDelete(e) => {
                    if e.unavailable {
                        cache.cache_unavailable_guild(e.id).await?
                    } else {
                        cache.delete_guild(e.id).await?
                    }
                }
                Event::GuildUpdate(e) => cache.cache_partial_guild(e).await?,
                Event::InteractionCreate(e) => {
                    let mut change = CacheChange::default();

                    if let (Some(guild_id), Some(member)) = (e.guild_id, &e.member) {
                        if let Some(ref user) = member.user {
                            change += cache.cache_partial_member(guild_id, member, user).await?;
                        }
                    } else if let Some(ref user) = e.user {
                        change += cache.cache_user(user).await?;
                    }

                    if let Some(InteractionData::ApplicationCommand(ref data)) = e.data {
                        if let Some(ref resolved) = data.resolved {
                            for user in resolved.users.values() {
                                if let Some(member) = resolved.members.get(&user.id) {
                                    if let Some(guild_id) = e.guild_id {
                                        change += cache
                                            .cache_interaction_member(guild_id, member, user)
                                            .await?;
                                    }
                                }
                            }

                            if let Some(guild_id) = e.guild_id {
                                change +=
                                    cache.cache_roles(guild_id, resolved.roles.values()).await?;
                            }
                        }
                    }

                    change
                }
                Event::MemberAdd(e) => cache.cache_member(e.guild_id, &e.member).await?,
                Event::MemberRemove(e) => cache.delete_member(e.guild_id, e.user.id).await?,
                Event::MemberUpdate(e) => cache.cache_member_update(e).await?,
                Event::MemberChunk(e) => cache.cache_members(e.guild_id, &e.members).await?,
                Event::MessageCreate(e) => match (e.guild_id, &e.member) {
                    (Some(guild_id), Some(member)) => {
                        cache
                            .cache_partial_member(guild_id, member, &e.author)
                            .await?
                    }
                    _ => cache.cache_user(&e.author).await?,
                },
                Event::MessageUpdate(e) => {
                    if let Some(ref user) = e.author {
                        cache.cache_user(user).await?
                    } else {
                        return Ok(None);
                    }
                }
                Event::Ready(e) => {
                    cache.cache_current_user(&e.user).await?;

                    let mut change = CacheChange::default();

                    for guild in e.guilds.iter().filter(|guild| guild.unavailable) {
                        change += cache.cache_unavailable_guild(guild.id).await?;
                    }

                    change
                }
                Event::RoleCreate(e) => cache.cache_role(e.guild_id, &e.role).await?,
                Event::RoleDelete(e) => cache.delete_role(e.guild_id, e.role_id).await?,
                Event::RoleUpdate(e) => cache.cache_role(e.guild_id, &e.role).await?,
                Event::ThreadCreate(e) => cache.cache_channel(e).await?,
                Event::ThreadDelete(e) => cache.delete_channel(Some(e.guild_id), e.id).await?,
                Event::ThreadListSync(e) => cache.cache_channels(e.guild_id, &e.threads).await?,
                Event::ThreadUpdate(e) => cache.cache_channel(e).await?,
                Event::UserUpdate(e) => return cache.cache_current_user(e).await.map(|_| None),
                _ => return Ok(None),
            };

            Ok(Some(change))
        }

        match update(self, event).await {
            Ok(Some(change)) => {
                self.stats.update(&change);

                Some(change)
            }
            Ok(None) => None,
            Err(err) => {
                let event = event.kind().name().unwrap_or("<unnamed>");
                error!(event, ?err, "Failed to update cache");

                None
            }
        }
    }

    pub fn stats(&self) -> CacheStats {
        self.stats.get()
    }

    pub(crate) async fn connection(&self) -> Result<PooledConnection<RedisConnectionManager>> {
        self.redis
            .get()
            .await
            .wrap_err("Failed to get redis connection")
    }
}
