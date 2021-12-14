use crate::{
    embeds::{EmbedBuilder, EmbedData, Footer},
    util::numbers::with_comma_int,
};

use chrono::{DateTime, Utc};
use std::fmt::Write;
use twilight_cache_inmemory::InMemoryCacheStats;

pub struct CacheEmbed {
    description: String,
    footer: Footer,
    timestamp: DateTime<Utc>,
}

impl CacheEmbed {
    pub fn new(stats: InMemoryCacheStats<'_>, start_time: DateTime<Utc>) -> Self {
        let mut description = String::with_capacity(256);

        let _ = writeln!(description, "Guilds: {}", with_comma_int(stats.guilds()));
        let _ = writeln!(description, "Members: {}", with_comma_int(stats.members()));
        let _ = writeln!(description, "Roles: {}", with_comma_int(stats.roles()));

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
