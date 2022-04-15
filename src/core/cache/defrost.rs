use std::time::Instant;

use bb8_redis::redis::AsyncCommands;
use futures::{future::Either, stream::FuturesUnordered, TryFutureExt, TryStreamExt};
use rkyv::{Deserialize, Infallible};
use twilight_cache_inmemory::{
    model::{CachedGuild, CachedMember},
    GuildResource,
};
use twilight_model::{
    channel::Channel,
    guild::Role,
    user::{CurrentUser, User},
};

use super::{
    Cache, ColdResumeData, ColdResumeErrorKind, DefrostError, DefrostInnerError, Redis, ResumeData,
    CHANNEL_KEY_PREFIX, CURRENT_USER_KEY, DATA_KEY, GUILD_KEY_PREFIX, MEMBER_KEY_PREFIX,
    ROLE_KEY_PREFIX, USER_KEY_PREFIX,
};

impl Cache {
    pub(super) async fn defrost(&self, redis: &Redis) -> Result<ResumeData, DefrostError> {
        let start = Instant::now();

        let data = self
            .defrost_resume_data(redis)
            .await
            .map_err(|inner| DefrostError {
                kind: ColdResumeErrorKind::ResumeData,
                inner,
            })?;

        let futs = FuturesUnordered::new();

        let fut = self
            .defrost_guilds(redis, data.guild_chunks)
            .map_err(|inner| DefrostError {
                kind: ColdResumeErrorKind::Guilds,
                inner,
            });

        futs.push(Either::Left(fut));

        let fut = self
            .defrost_users(redis, data.user_chunks)
            .map_err(|inner| DefrostError {
                kind: ColdResumeErrorKind::Users,
                inner,
            });

        futs.push(Either::Right(Either::Left(fut)));

        let fut = self
            .defrost_members(redis, data.member_chunks)
            .map_err(|inner| DefrostError {
                kind: ColdResumeErrorKind::Members,
                inner,
            });

        futs.push(Either::Right(Either::Right(Either::Left(fut))));

        let fut = self
            .defrost_roles(redis, data.role_chunks)
            .map_err(|inner| DefrostError {
                kind: ColdResumeErrorKind::Roles,
                inner,
            });

        futs.push(Either::Right(Either::Right(Either::Right(Either::Left(
            fut,
        )))));

        let fut = self
            .defrost_channels(redis, data.channel_chunks)
            .map_err(|inner| DefrostError {
                kind: ColdResumeErrorKind::Channels,
                inner,
            });

        futs.push(Either::Right(Either::Right(Either::Right(Either::Right(
            Either::Left(fut),
        )))));

        let fut = self
            .defrost_current_user(redis)
            .map_err(|inner| DefrostError {
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

    async fn defrost_resume_data(
        &self,
        redis: &Redis,
    ) -> Result<ColdResumeData, DefrostInnerError> {
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

    async fn defrost_current_user(&self, redis: &Redis) -> Result<(), DefrostInnerError> {
        let mut conn = redis.get().await?;
        let bytes: Vec<u8> = conn.get(CURRENT_USER_KEY).await?;

        if bytes.is_empty() {
            return Err(DefrostInnerError::MissingKey(CURRENT_USER_KEY.to_owned()));
        }

        let archived = unsafe { rkyv::archived_root::<CurrentUser>(&bytes) };
        conn.del(CURRENT_USER_KEY).await?;

        let user = Deserialize::<CurrentUser, _>::deserialize(archived, &mut Infallible).unwrap();
        self.inner.cache_current_user(user);

        Ok(())
    }

    async fn defrost_channels(
        &self,
        redis: &Redis,
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
                self.inner.insert_channel(channel);
            }
        }

        Ok(())
    }

    async fn defrost_roles(&self, redis: &Redis, chunks: usize) -> Result<(), DefrostInnerError> {
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
                self.inner.insert_role(role);
            }
        }

        Ok(())
    }

    async fn defrost_members(&self, redis: &Redis, chunks: usize) -> Result<(), DefrostInnerError> {
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
                self.inner.insert_member(member);
            }
        }

        Ok(())
    }

    async fn defrost_users(&self, redis: &Redis, chunks: usize) -> Result<(), DefrostInnerError> {
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
                self.inner.insert_user(user);
            }
        }

        Ok(())
    }

    async fn defrost_guilds(&self, redis: &Redis, chunks: usize) -> Result<(), DefrostInnerError> {
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
                self.inner.insert_guild(guild);
            }
        }

        Ok(())
    }
}
