mod get_impls;

use darkredis::ConnectionPool;
use std::{collections::HashMap, ops::Deref};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::shard::ResumeSession;

pub struct Cache(InMemoryCache);

impl Cache {
    pub async fn new(redis: &ConnectionPool) -> (Self, Option<HashMap<u64, ResumeSession>>) {
        let resource_types = ResourceType::CHANNEL
            | ResourceType::GUILD
            | ResourceType::MESSAGE
            | ResourceType::MEMBER
            | ResourceType::REACTION
            | ResourceType::ROLE
            | ResourceType::USER_CURRENT
            | ResourceType::USER;

        let config = InMemoryCache::builder()
            .message_cache_size(5)
            .resource_types(resource_types)
            .build()
            .config();

        let (cache, resume_map) = InMemoryCache::from_redis(redis, config).await;

        (Self(cache), resume_map)
    }
}

impl Deref for Cache {
    type Target = InMemoryCache;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
