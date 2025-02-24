use bb8_redis::{RedisConnectionManager, bb8::PooledConnection};

/// Provided by `Cache::fetch` to be later used in `Cache::cache_data`.
pub struct CacheConnection<'c>(pub(crate) PooledConnection<'c, RedisConnectionManager>);
