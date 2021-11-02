use std::{collections::HashMap, ops::Deref};

use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::shard::ResumeSession;
use twilight_model::id::{GuildId, UserId};

pub struct Cache(InMemoryCache);

impl Cache {
    pub async fn new() -> (Self, Option<HashMap<u64, ResumeSession>>) {
        let resource_types = ResourceType::CHANNEL
            | ResourceType::GUILD
            | ResourceType::MEMBER
            | ResourceType::REACTION
            | ResourceType::ROLE
            | ResourceType::USER_CURRENT
            | ResourceType::USER;

        let cache = InMemoryCache::builder()
            .resource_types(resource_types)
            .build();

        (Self(cache), None)
    }

    pub fn is_guild_owner(&self, guild_id: GuildId, user_id: UserId) -> bool {
        self.0
            .guild(guild_id)
            .map_or(false, |guild| guild.owner_id() == user_id)
    }
}

impl Deref for Cache {
    type Target = InMemoryCache;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
