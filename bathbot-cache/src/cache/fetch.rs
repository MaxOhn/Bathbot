use bathbot_model::twilight::{
    channel::CachedChannel,
    guild::{CachedGuild, CachedMember, CachedRole},
    user::{CachedCurrentUser, CachedUser},
};
use bb8_redis::redis::AsyncCommands;
use eyre::{Result, WrapErr};
use rkyv::{bytecheck::CheckBytes, with::ArchiveWith, Archive};
use twilight_model::id::{
    marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker},
    Id,
};

use crate::{
    key::{RedisKey, ToCacheKey},
    model::{CacheConnection, CachedArchive, ValidatorStrategy},
    util::AlignedVecRedisArgs,
    Cache,
};

type FetchResult<T> = Result<Option<CachedArchive<T>>>;

impl Cache {
    #[inline]
    pub async fn fetch<K, T>(&self, key: &K) -> Result<Result<CachedArchive<T>, CacheConnection>>
    where
        K: ToCacheKey + ?Sized,
        T: for<'a> Archive<Archived: CheckBytes<ValidatorStrategy<'a>>>,
    {
        let mut conn = self.connection().await?;

        conn.get::<_, Option<AlignedVecRedisArgs>>(RedisKey::from(key))
            .await
            .map(|opt| match opt {
                Some(AlignedVecRedisArgs(bytes)) => Ok(CachedArchive::new(bytes)),
                None => Err(CacheConnection(conn)),
            })
            .wrap_err("Failed to fetch stored data")
    }

    #[inline]
    pub async fn fetch_with<K, T, W>(
        &self,
        key: &K,
    ) -> Result<Result<CachedArchive<T>, CacheConnection>>
    where
        K: ToCacheKey + ?Sized,
        T: ?Sized,
        W: for<'a> ArchiveWith<T, Archived: CheckBytes<ValidatorStrategy<'a>>>,
    {
        let mut conn = self.connection().await?;

        conn.get::<_, Option<AlignedVecRedisArgs>>(RedisKey::from(key))
            .await
            .map(|opt| match opt {
                Some(AlignedVecRedisArgs(bytes)) => Ok(CachedArchive::new_with::<W>(bytes)),
                None => Err(CacheConnection(conn)),
            })
            .wrap_err("Failed to fetch stored data")
    }

    #[inline]
    pub async fn channel(
        &self,
        guild: Option<Id<GuildMarker>>,
        channel: Id<ChannelMarker>,
    ) -> FetchResult<CachedChannel> {
        self.connection()
            .await?
            .get(RedisKey::channel(guild, channel))
            .await
            .map(|opt: Option<_>| opt.map(|AlignedVecRedisArgs(bytes)| CachedArchive::new(bytes)))
            .wrap_err("Failed to get stored channel")
    }

    #[inline]
    pub async fn current_user(&self) -> FetchResult<CachedCurrentUser> {
        self.connection()
            .await?
            .get(RedisKey::current_user())
            .await
            .map(|opt: Option<_>| opt.map(|AlignedVecRedisArgs(bytes)| CachedArchive::new(bytes)))
            .wrap_err("Failed to get stored current user")
    }

    #[inline]
    pub async fn guild(&self, guild: Id<GuildMarker>) -> FetchResult<CachedGuild> {
        self.connection()
            .await?
            .get(RedisKey::guild(guild))
            .await
            .map(|opt: Option<_>| opt.map(|AlignedVecRedisArgs(bytes)| CachedArchive::new(bytes)))
            .wrap_err("Failed to get stored guild")
    }

    #[inline]
    pub async fn members(&self, guild: Id<GuildMarker>) -> Result<Vec<u64>> {
        self.connection()
            .await?
            .smembers(RedisKey::guild_members(guild))
            .await
            .wrap_err("Failed to get member ids")
    }

    #[inline]
    pub async fn member(
        &self,
        guild: Id<GuildMarker>,
        user: Id<UserMarker>,
    ) -> FetchResult<CachedMember> {
        self.connection()
            .await?
            .get(RedisKey::member(guild, user))
            .await
            .map(|opt: Option<_>| opt.map(|AlignedVecRedisArgs(bytes)| CachedArchive::new(bytes)))
            .wrap_err("Failed to get stored member")
    }

    #[inline]
    pub async fn role(
        &self,
        guild: Id<GuildMarker>,
        role: Id<RoleMarker>,
    ) -> FetchResult<CachedRole> {
        self.connection()
            .await?
            .get(RedisKey::role(guild, role))
            .await
            .map(|opt: Option<_>| opt.map(|AlignedVecRedisArgs(bytes)| CachedArchive::new(bytes)))
            .wrap_err("Failed to get stored role")
    }

    #[inline]
    pub async fn roles<I>(
        &self,
        guild: Id<GuildMarker>,
        roles: I,
    ) -> Result<Vec<CachedArchive<CachedRole>>>
    where
        I: IntoIterator<Item = Id<RoleMarker>>,
    {
        let keys: Vec<_> = roles
            .into_iter()
            .map(|role| RedisKey::role(guild, role))
            .collect();

        self.connection()
            .await?
            .mget(keys)
            .await
            .map(|items: Vec<_>| {
                items
                    .into_iter()
                    .map(|AlignedVecRedisArgs(bytes)| CachedArchive::new(bytes))
                    .collect()
            })
            .wrap_err("Failed to get stored roles")
    }

    #[inline]
    pub async fn user(&self, user: Id<UserMarker>) -> FetchResult<CachedUser> {
        self.connection()
            .await?
            .get(RedisKey::user(user))
            .await
            .map(|opt: Option<_>| opt.map(|AlignedVecRedisArgs(bytes)| CachedArchive::new(bytes)))
            .wrap_err("Failed to get stored user")
    }
}
