mod get_impls;

use darkredis::ConnectionPool;
use std::{collections::HashMap, ops::Deref};
use twilight_cache_inmemory::{EventType, InMemoryCache};
use twilight_gateway::shard::ResumeSession;

pub struct Cache(InMemoryCache);

impl Cache {
    pub async fn new(
        redis: &ConnectionPool,
        total_shards: u64,
        shards_per_cluster: u64,
    ) -> (Self, Option<HashMap<u64, ResumeSession>>) {
        let events = EventType::CHANNEL_CREATE
            | EventType::CHANNEL_DELETE
            | EventType::CHANNEL_UPDATE
            | EventType::GUILD_CREATE
            | EventType::GUILD_DELETE
            | EventType::GUILD_UPDATE
            | EventType::MEMBER_ADD
            | EventType::MEMBER_REMOVE
            | EventType::MEMBER_UPDATE
            | EventType::MEMBER_CHUNK
            | EventType::MESSAGE_CREATE
            | EventType::REACTION_ADD
            | EventType::REACTION_REMOVE
            | EventType::REACTION_REMOVE_ALL
            | EventType::READY
            | EventType::ROLE_CREATE
            | EventType::ROLE_DELETE
            | EventType::ROLE_UPDATE
            | EventType::UNAVAILABLE_GUILD
            | EventType::USER_UPDATE;
        let config = InMemoryCache::builder()
            .message_cache_size(10)
            .event_types(events)
            .build()
            .config();
        let (cache, resume_map) =
            InMemoryCache::from_redis(redis, total_shards, shards_per_cluster, config).await;
        (Self(cache), resume_map)
    }
}

impl Deref for Cache {
    type Target = InMemoryCache;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
