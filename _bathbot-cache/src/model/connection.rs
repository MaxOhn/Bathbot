use bb8_redis::{bb8::PooledConnection, RedisConnectionManager};

/// Provided by `Cache::fetch` to be later used in `Cache::cache_data`.
pub struct CacheConnection<'c>(pub(crate) PooledConnection<'c, RedisConnectionManager>);
