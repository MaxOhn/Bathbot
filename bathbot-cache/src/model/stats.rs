use bb8_redis::redis::AsyncCommands;
use eyre::{Result, WrapErr};

use crate::{key::RedisKey, Cache};

#[derive(Copy, Clone)]
pub struct CacheStats<'c> {
    cache: &'c Cache,
}

macro_rules! get_stat {
    ($name:ident, $key_fn:ident) => {
        pub async fn $name(&self) -> Result<usize> {
            self.cache
                .connection()
                .await?
                .scard(RedisKey::$key_fn())
                .await
                .wrap_err(concat!(
                    "Failed to get ",
                    stringify!($name),
                    " set cardinality"
                ))
        }
    };
}

impl<'c> CacheStats<'c> {
    pub fn new(cache: &'c Cache) -> Self {
        Self { cache }
    }

    get_stat!(channels, channel_ids_key);
    get_stat!(guilds, guild_ids_key);
    get_stat!(roles, role_ids_key);
    get_stat!(unavailable_guilds, unavailable_guild_ids_key);
    get_stat!(users, user_ids_key);
}
