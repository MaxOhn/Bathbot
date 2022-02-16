use crate::{
    embeds::{EmbedBuilder, EmbedData, Footer},
    util::numbers::with_comma_int,
};

use chrono::{DateTime, Utc};
use twilight_cache_inmemory::InMemoryCacheStats;

pub struct CacheEmbed {
    description: String,
    footer: Footer,
    timestamp: DateTime<Utc>,
}

impl CacheEmbed {
    pub fn new(stats: InMemoryCacheStats<'_>, start_time: DateTime<Utc>) -> Self {
        let description = format!(
            "Guilds: {guilds}\n\
            Members: {members}\n\
            Users: {users}\n\
            Roles: {roles}\n\
            Channels: {channels}",
            guilds = with_comma_int(stats.guilds()),
            members = with_comma_int(stats.members()),
            users = with_comma_int(stats.users()),
            roles = with_comma_int(stats.roles()),
            channels = with_comma_int(stats.guild_channels_total()),
        );

        Self {
            description,
            footer: Footer::new("Boot time"),
            timestamp: start_time,
        }
    }
}

impl EmbedData for CacheEmbed {
    fn into_builder(self) -> EmbedBuilder {
        EmbedBuilder::new()
            .description(self.description)
            .footer(self.footer)
            .timestamp(self.timestamp)
    }
}
