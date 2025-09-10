use bathbot_model::twilight::{
    channel::ArchivedCachedChannel,
    guild::{ArchivedCachedGuild, ArchivedCachedMember, ArchivedCachedRole},
    user::{ArchivedCachedCurrentUser, ArchivedCachedUser},
};
use bb8_redis::{
    bb8::RunError,
    redis::{AsyncCommands, RedisError},
};
use eyre::{Report, WrapErr};
use rkyv::{Portable, bytecheck::CheckBytes, rancor::BoxedError};
use thiserror::Error as ThisError;
use twilight_model::id::{
    Id,
    marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker},
};

use crate::{
    Cache,
    key::{RedisKey, ToCacheKey},
    model::{CacheConnection, CachedArchive, ValidatorStrategy},
    util::AlignedVecRedisArgs,
};

type FetchResult<T> = Result<Option<CachedArchive<T>>, FetchError>;

impl Cache {
    pub async fn fetch<K, T>(
        &self,
        key: &K,
    ) -> Result<Result<CachedArchive<T>, CacheConnection<'_>>, FetchError>
    where
        K: ToCacheKey + ?Sized,
        T: Portable + for<'a> CheckBytes<ValidatorStrategy<'a>>,
    {
        let mut conn = self.connection().await?;

        let Some(AlignedVecRedisArgs(bytes)) = conn.get(RedisKey::from(key)).await? else {
            return Ok(Err(CacheConnection(conn)));
        };

        Ok(Ok(CachedArchive::new(bytes)?))
    }

    pub async fn fetch_raw<K>(
        &self,
        key: &K,
    ) -> Result<Result<Vec<u8>, CacheConnection<'_>>, FetchError>
    where
        K: ToCacheKey + ?Sized,
    {
        let mut conn = self.connection().await?;

        let Some(bytes) = conn.get(RedisKey::from(key)).await? else {
            return Ok(Err(CacheConnection(conn)));
        };

        Ok(Ok(bytes))
    }

    async fn fetch_discord_type<T>(&self, key: RedisKey<'_>) -> FetchResult<T>
    where
        T: Portable + Portable + for<'a> CheckBytes<ValidatorStrategy<'a>>,
    {
        let Some(AlignedVecRedisArgs(bytes)) = self.connection().await?.get(key).await? else {
            return Ok(None);
        };

        Ok(Some(CachedArchive::new(bytes)?))
    }

    pub async fn channel(
        &self,
        guild: Option<Id<GuildMarker>>,
        channel: Id<ChannelMarker>,
    ) -> FetchResult<ArchivedCachedChannel> {
        self.fetch_discord_type(RedisKey::channel(guild, channel))
            .await
    }

    pub async fn current_user(&self) -> FetchResult<ArchivedCachedCurrentUser<'_>> {
        self.fetch_discord_type(RedisKey::current_user()).await
    }

    pub async fn guild(&self, guild: Id<GuildMarker>) -> FetchResult<ArchivedCachedGuild> {
        self.fetch_discord_type(RedisKey::guild(guild)).await
    }

    pub async fn members(&self, guild: Id<GuildMarker>) -> Result<Vec<u64>, Report> {
        self.connection()
            .await
            .map_err(FetchError::Connection)
            .map_err(Report::new)?
            .smembers(RedisKey::guild_members(guild))
            .await
            .wrap_err("Failed to get member ids")
    }

    pub async fn member(
        &self,
        guild: Id<GuildMarker>,
        user: Id<UserMarker>,
    ) -> FetchResult<ArchivedCachedMember> {
        self.fetch_discord_type(RedisKey::member(guild, user)).await
    }

    pub async fn role(
        &self,
        guild: Id<GuildMarker>,
        role: Id<RoleMarker>,
    ) -> FetchResult<ArchivedCachedRole<'_>> {
        self.fetch_discord_type(RedisKey::role(guild, role)).await
    }

    pub async fn roles<I>(
        &self,
        guild: Id<GuildMarker>,
        roles: I,
    ) -> Result<Vec<CachedArchive<ArchivedCachedRole<'_>>>, FetchError>
    where
        I: IntoIterator<Item = Id<RoleMarker>>,
    {
        let keys: Vec<_> = roles
            .into_iter()
            .map(|role| RedisKey::role(guild, role))
            .collect();

        let items: Vec<_> = self.connection().await?.mget(keys).await?;

        items
            .into_iter()
            .map(|AlignedVecRedisArgs(bytes)| CachedArchive::new(bytes))
            .collect::<Result<_, _>>()
            .map_err(FetchError::Validation)
    }

    pub async fn user(&self, user: Id<UserMarker>) -> FetchResult<ArchivedCachedUser> {
        self.fetch_discord_type(RedisKey::user(user)).await
    }
}

#[derive(Debug, ThisError)]
pub enum FetchError {
    #[error("Failed to acquire connection")]
    Connection(#[from] RunError<RedisError>),
    #[error("Redis error")]
    Redis(#[from] RedisError),
    #[error("Validation error")]
    Validation(#[from] BoxedError),
}
