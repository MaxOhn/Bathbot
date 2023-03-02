use bb8_redis::redis::AsyncCommands;
use eyre::{Result, WrapErr};
use twilight_model::{
    channel::Channel,
    guild::Role,
    id::{
        marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker},
        Id,
    },
    user::{CurrentUser, User},
};

use crate::{
    key::{IntoCacheKey, RedisKey},
    model::{CacheConnection, CachedArchive, CachedGuild, CachedMember},
    Cache,
};

type FetchResult<T> = Result<Option<CachedArchive<T>>>;

impl Cache {
    #[inline]
    pub async fn fetch<'k, K, T>(&self, key: K) -> Result<Result<CachedArchive<T>, CacheConnection>>
    where
        K: IntoCacheKey<'k>,
    {
        let mut conn = self.connection().await?;

        conn.get::<_, Option<CachedArchive<T>>>(RedisKey::from(key))
            .await
            .map(|archived| archived.ok_or(CacheConnection(conn)))
            .wrap_err("Failed to fetch stored data")
    }

    #[inline]
    pub async fn channel(
        &self,
        guild: Option<Id<GuildMarker>>,
        channel: Id<ChannelMarker>,
    ) -> FetchResult<Channel> {
        self.connection()
            .await?
            .get(RedisKey::channel(guild, channel))
            .await
            .wrap_err("Failed to get stored channel")
    }

    #[inline]
    pub async fn current_user(&self) -> FetchResult<CurrentUser> {
        self.connection()
            .await?
            .get(RedisKey::CurrentUser)
            .await
            .wrap_err("Failed to get stored current user")
    }

    #[inline]
    pub async fn guild(&self, guild: Id<GuildMarker>) -> FetchResult<CachedGuild<'static>> {
        self.connection()
            .await?
            .get(RedisKey::guild(guild))
            .await
            .wrap_err("Failed to get stored guild")
    }

    #[inline]
    pub async fn members(&self, guild: Id<GuildMarker>) -> Result<Vec<u64>> {
        self.connection()
            .await?
            .smembers(RedisKey::guild_members_key(guild))
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
            .wrap_err("Failed to get stored member")
    }

    #[inline]
    pub async fn role(&self, guild: Id<GuildMarker>, role: Id<RoleMarker>) -> FetchResult<Role> {
        self.connection()
            .await?
            .get(RedisKey::role(guild, role))
            .await
            .wrap_err("Failed to get stored role")
    }

    #[inline]
    pub async fn roles<I>(
        &self,
        guild: Id<GuildMarker>,
        roles: I,
    ) -> Result<Vec<CachedArchive<Role>>>
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
            .wrap_err("Failed to get stored roles")
    }

    #[inline]
    pub async fn user(&self, user: Id<UserMarker>) -> FetchResult<User> {
        self.connection()
            .await?
            .get(RedisKey::user(user))
            .await
            .wrap_err("Failed to get stored user")
    }
}
