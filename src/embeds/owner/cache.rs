use crate::{
    embeds::{EmbedBuilder, EmbedData, Footer},
    util::numbers::with_comma_uint,
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
    pub fn new(stats: InMemoryCacheStats, start_time: DateTime<Utc>) -> Self {
        let mut description = String::with_capacity(256);

        let metrics = stats.cache_ref().metrics();

        let _ = writeln!(
            description,
            "Guild channels: {}",
            with_comma_uint(metrics.channels_guild.get() as u64)
        );

        let _ = writeln!(
            description,
            "Private channels: {}",
            with_comma_uint(stats.private_channels())
        );

        let _ = writeln!(description, "Emojis: {}", with_comma_uint(stats.emojis()));
        let _ = writeln!(description, "Guilds: {}", with_comma_uint(stats.guilds()));
        let _ = writeln!(description, "Members: {}", with_comma_uint(stats.members()));
        let _ = writeln!(description, "Roles: {}", with_comma_uint(stats.roles()));

        let _ = writeln!(
            description,
            "Unavailable guilds: {}",
            stats.unavailable_guilds()
        );

        let _ = writeln!(description, "Users: {}", stats.users());
        let _ = writeln!(description, "Groups: {}", with_comma_uint(stats.groups()));
        let _ = writeln!(description, "Presences: {}", stats.presences());
        let _ = writeln!(description, "Voice states: {}", stats.voice_states());

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
