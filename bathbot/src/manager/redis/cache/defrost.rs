use std::time::Instant;

use bb8_redis::{bb8::Pool, redis::AsyncCommands, RedisConnectionManager};
use eyre::Result;
use futures::{future::Either, stream::FuturesUnordered, TryFutureExt, TryStreamExt};
use rkyv::{Deserialize, Infallible};
use twilight_cache_inmemory::{
    model::{CachedGuild, CachedMember},
    GuildResource, InMemoryCache as Cache,
};
use twilight_model::{
    channel::Channel,
    guild::Role,
    user::{CurrentUser, User},
};

use crate::manager::redis::RedisManager;

use super::{
    error::{ColdResumeErrorKind, DefrostError, DefrostInnerError},
    ColdResumeData, ResumeData, CHANNEL_KEY_PREFIX, CURRENT_USER_KEY, DATA_KEY, GUILD_KEY_PREFIX,
    MEMBER_KEY_PREFIX, ROLE_KEY_PREFIX, USER_KEY_PREFIX,
};

type Redis = Pool<RedisConnectionManager>;

impl RedisManager<'_> {
    pub async fn defrost_cache(redis: &Redis, cache: &Cache) -> Result<ResumeData, DefrostError> {
        let start = Instant::now();

        let data = Self::defrost_resume_data(redis)
            .await
            .map_err(|inner| DefrostError {
                kind: ColdResumeErrorKind::ResumeData,
                inner,
            })?;

        let futs = FuturesUnordered::new();

        let fut =
            Self::defrost_guilds(redis, cache, data.guild_chunks).map_err(|inner| DefrostError {
                kind: ColdResumeErrorKind::Guilds,
                inner,
            });

        futs.push(Either::Left(fut));

        let fut =
            Self::defrost_users(redis, cache, data.user_chunks).map_err(|inner| DefrostError {
                kind: ColdResumeErrorKind::Users,
                inner,
            });

        futs.push(Either::Right(Either::Left(fut)));

        let fut =
            Self::defrost_members(redis, cache, data.member_chunks).map_err(|inner| DefrostError {
                kind: ColdResumeErrorKind::Members,
                inner,
            });

        futs.push(Either::Right(Either::Right(Either::Left(fut))));

        let fut =
            Self::defrost_roles(redis, cache, data.role_chunks).map_err(|inner| DefrostError {
                kind: ColdResumeErrorKind::Roles,
                inner,
            });

        futs.push(Either::Right(Either::Right(Either::Right(Either::Left(
            fut,
        )))));

        let fut = Self::defrost_channels(redis, cache, data.channel_chunks).map_err(|inner| {
            DefrostError {
                kind: ColdResumeErrorKind::Channels,
                inner,
            }
        });

        futs.push(Either::Right(Either::Right(Either::Right(Either::Right(
            Either::Left(fut),
        )))));

        let fut = Self::defrost_current_user(redis, cache).map_err(|inner| DefrostError {
            kind: ColdResumeErrorKind::CurrentUser,
            inner,
        });

        futs.push(Either::Right(Either::Right(Either::Right(Either::Right(
            Either::Right(fut),
        )))));

        futs.try_collect().await?;

        info!("Successfully defrosted cache [{:?}]", start.elapsed());

        Ok(data.resume_data)
    }

    async fn defrost_resume_data(redis: &Redis) -> Result<ColdResumeData, DefrostInnerError> {
        let mut conn = redis.get().await?;
        let bytes: Vec<u8> = conn.get(DATA_KEY).await?;

        if bytes.is_empty() {
            return Err(DefrostInnerError::MissingKey(DATA_KEY.to_owned()));
        }

        let archived = unsafe { rkyv::archived_root::<ColdResumeData>(&bytes) };
        conn.del(DATA_KEY).await?;

        let data =
            Deserialize::<ColdResumeData, _>::deserialize(archived, &mut Infallible).unwrap();

        Ok(data)
    }

    async fn defrost_current_user(redis: &Redis, cache: &Cache) -> Result<(), DefrostInnerError> {
        let mut conn = redis.get().await?;
        let bytes: Vec<u8> = conn.get(CURRENT_USER_KEY).await?;

        if bytes.is_empty() {
            return Err(DefrostInnerError::MissingKey(CURRENT_USER_KEY.to_owned()));
        }

        let archived = unsafe { rkyv::archived_root::<CurrentUser>(&bytes) };
        conn.del(CURRENT_USER_KEY).await?;

        let user = Deserialize::<CurrentUser, _>::deserialize(archived, &mut Infallible).unwrap();
        cache.cache_current_user(user);

        Ok(())
    }

    async fn defrost_channels(
        redis: &Redis,
        cache: &Cache,
        chunks: usize,
    ) -> Result<(), DefrostInnerError> {
        let mut conn = redis.get().await?;

        for idx in 0..chunks {
            let key = format!("{CHANNEL_KEY_PREFIX}_{idx}");
            let bytes: Vec<u8> = conn.get(&key).await?;

            if bytes.is_empty() {
                return Err(DefrostInnerError::MissingKey(key));
            }

            let channels = unsafe { rkyv::archived_root::<Vec<Channel>>(&bytes) };
            conn.del(key).await?;

            debug!(
                "Channels worker {idx} found {} channels to defrost",
                channels.len()
            );

            for channel in channels.iter() {
                let channel =
                    Deserialize::<Channel, _>::deserialize(channel, &mut Infallible).unwrap();
                cache.insert_channel(channel);
            }
        }

        Ok(())
    }

    async fn defrost_roles(
        redis: &Redis,
        cache: &Cache,
        chunks: usize,
    ) -> Result<(), DefrostInnerError> {
        let mut conn = redis.get().await?;

        for idx in 0..chunks {
            let key = format!("{ROLE_KEY_PREFIX}_{idx}");
            let bytes: Vec<u8> = conn.get(&key).await?;

            if bytes.is_empty() {
                return Err(DefrostInnerError::MissingKey(key));
            }

            let roles = unsafe { rkyv::archived_root::<Vec<GuildResource<Role>>>(&bytes) };
            conn.del(key).await?;

            debug!("Roles worker {idx} found {} roles to defrost", roles.len());

            for role in roles.iter() {
                let role =
                    Deserialize::<GuildResource<Role>, _>::deserialize(role, &mut Infallible)
                        .unwrap();
                cache.insert_role(role);
            }
        }

        Ok(())
    }

    async fn defrost_members(
        redis: &Redis,
        cache: &Cache,
        chunks: usize,
    ) -> Result<(), DefrostInnerError> {
        let mut conn = redis.get().await?;

        for idx in 0..chunks {
            let key = format!("{MEMBER_KEY_PREFIX}_{idx}");
            let bytes: Vec<u8> = conn.get(&key).await?;

            if bytes.is_empty() {
                return Err(DefrostInnerError::MissingKey(key));
            }

            let members = unsafe { rkyv::archived_root::<Vec<CachedMember>>(&bytes) };
            conn.del(key).await?;

            debug!(
                "Members worker {idx} found {} members to defrost",
                members.len()
            );

            for member in members.iter() {
                let member =
                    Deserialize::<CachedMember, _>::deserialize(member, &mut Infallible).unwrap();
                cache.insert_member(member);
            }
        }

        Ok(())
    }

    async fn defrost_users(
        redis: &Redis,
        cache: &Cache,
        chunks: usize,
    ) -> Result<(), DefrostInnerError> {
        let mut conn = redis.get().await?;

        for idx in 0..chunks {
            let key = format!("{USER_KEY_PREFIX}_{idx}");
            let bytes: Vec<u8> = conn.get(&key).await?;

            if bytes.is_empty() {
                return Err(DefrostInnerError::MissingKey(key));
            }

            let users = unsafe { rkyv::archived_root::<Vec<User>>(&bytes) };
            conn.del(key).await?;

            debug!("Users worker {idx} found {} users to defrost", users.len());

            for user in users.iter() {
                let user = Deserialize::<User, _>::deserialize(user, &mut Infallible).unwrap();
                cache.insert_user(user);
            }
        }

        Ok(())
    }

    async fn defrost_guilds(
        redis: &Redis,
        cache: &Cache,
        chunks: usize,
    ) -> Result<(), DefrostInnerError> {
        let mut conn = redis.get().await?;

        for idx in 0..chunks {
            let key = format!("{GUILD_KEY_PREFIX}_{idx}");
            let bytes: Vec<u8> = conn.get(&key).await?;

            if bytes.is_empty() {
                return Err(DefrostInnerError::MissingKey(key));
            }

            let guilds = unsafe { rkyv::archived_root::<Vec<CachedGuild>>(&bytes) };
            conn.del(key).await?;

            debug!(
                "Guild worker {idx} found {} guilds to defrost",
                guilds.len()
            );

            for guild in guilds.iter() {
                let guild =
                    Deserialize::<CachedGuild, _>::deserialize(guild, &mut Infallible).unwrap();
                cache.insert_guild(guild);
            }
        }

        Ok(())
    }
}
