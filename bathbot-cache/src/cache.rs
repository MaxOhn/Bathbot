use std::{collections::HashSet, convert::identity, ops::Deref, time::Duration};

use bb8_redis::{bb8::PooledConnection, redis::AsyncCommands, RedisConnectionManager};
use eyre::{Report, Result, WrapErr};
use redlight::{CachedArchive as RedlightArchive, RedisCache};
use twilight_model::id::{
    marker::{GuildMarker, RoleMarker, UserMarker},
    Id,
};

use crate::{
    config::Config, data::BathbotRedisData, twilight::role::CachedRole, util::BytesWrap,
    value::CachedArchive,
};

pub struct Cache {
    inner: RedisCache<Config>,
}

impl Cache {
    pub async fn new(url: &str) -> Result<Self> {
        let inner = RedisCache::new(url)
            .await
            .wrap_err("Failed to create cache")?;

        Ok(Self { inner })
    }

    async fn connection(&self) -> Result<PooledConnection<'_, RedisConnectionManager>> {
        self.inner
            .pool()
            .get()
            .await
            .wrap_err("Failed to get a connection")
    }

    pub async fn store<T: BathbotRedisData>(&self, key: &str, value: &T::Original) -> Result<()> {
        let bytes = T::serialize(value).wrap_err("Failed to serialize data")?;

        self.store_serialized::<T>(key, bytes.as_ref()).await
    }

    pub async fn store_serialized<T: BathbotRedisData>(
        &self,
        key: &str,
        bytes: &[u8],
    ) -> Result<()> {
        self.store_raw(key, T::EXPIRE, bytes).await
    }

    async fn store_raw(&self, key: &str, expire: Option<Duration>, bytes: &[u8]) -> Result<()> {
        let mut conn = self.connection().await?;

        let _: () = if let Some(expire) = expire {
            conn.set_ex(key, bytes.as_ref(), expire.as_secs() as usize)
                .await?
        } else {
            conn.set(key, bytes.as_ref()).await?
        };

        Ok(())
    }

    pub async fn fetch<T>(&self, key: &str) -> Result<Option<CachedArchive<T::Archived>>>
    where
        T: BathbotRedisData,
    {
        let mut conn = self.connection().await?;

        let bytes = match conn.get(key).await {
            Ok(Some(BytesWrap(bytes))) => bytes,
            Ok(None) => return Ok(None),
            Err(err) => return Err(Report::new(err).wrap_err("Failed to fetch data")),
        };

        CachedArchive::<T::Archived>::new(bytes)
            .wrap_err("Failed to validate data")
            .map(Some)
    }

    /// Insert a value into a set.
    ///
    /// Returns whether the value was newly inserted. That is:
    ///
    /// - If the set did not previously contain this value, `true` is returned.
    /// - If the set already contained this value, `false` is returned.
    ///
    /// The only current use is for values of type `u64`. If other use-cases
    /// arise, this type should be adjusted.
    pub async fn insert_into_set(&self, key: &str, value: u64) -> Result<bool> {
        let count: u8 = self.connection().await?.sadd(key, value).await?;

        Ok(count == 1)
    }

    pub async fn member_ids(&self, guild_id: Id<GuildMarker>) -> Result<HashSet<Id<UserMarker>>> {
        self.inner
            .guild_member_ids(guild_id)
            .await
            .wrap_err("Failed to fetch member ids")
    }

    pub async fn roles<I>(&self, ids: I) -> Result<Vec<RedlightArchive<CachedRole>>>
    where
        I: IntoIterator<Item = Id<RoleMarker>>,
    {
        let iter = self.inner.iter().roles_by_ids(ids).await?;

        iter.filter_map(identity)
            .collect::<Result<Vec<_>, _>>()
            .wrap_err("Failed to validate data")
    }
}

impl Deref for Cache {
    type Target = RedisCache<Config>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
