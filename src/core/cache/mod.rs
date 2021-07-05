mod get_impls;

use deadpool_redis::Pool;
use std::{collections::HashMap, ops::Deref};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::shard::ResumeSession;

pub struct Cache(InMemoryCache);

impl Cache {
    pub async fn new(redis: &Pool) -> (Self, Option<HashMap<u64, ResumeSession>>) {
        let resource_types = ResourceType::CHANNEL
            | ResourceType::GUILD
            | ResourceType::MEMBER
            | ResourceType::MESSAGE
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

        let resume_map = resume_map.map(|resume_map| {
            resume_map
                .into_iter()
                .map(|(key, (session_id, sequence))| {
                    let session = ResumeSession {
                        session_id,
                        sequence,
                    };

                    (key, session)
                })
                .collect()
        });

        (Self(cache), resume_map)
    }
}

impl Deref for Cache {
    type Target = InMemoryCache;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
