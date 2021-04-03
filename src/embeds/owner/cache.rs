use crate::{
    embeds::{EmbedBuilder, EmbedData, EmbedFields, Footer},
    util::numbers::with_comma_uint,
};

use chrono::{DateTime, Utc};
use std::{fmt::Write, sync::atomic::Ordering::Relaxed};
use twilight_cache_inmemory::CacheStats;

pub struct CacheEmbed {
    description: String,
    fields: EmbedFields,
    footer: Footer,
    timestamp: DateTime<Utc>,
}

impl CacheEmbed {
    pub fn new(stats: CacheStats, start_time: DateTime<Utc>) -> Self {
        let mut description = String::with_capacity(256);

        // Note: As inaccuracies in displaying these stats is non-critical,
        // lifting restrictions by using Ordering::Relaxed is fine.

        let _ = writeln!(
            description,
            "Channels (Guilds): {}",
            with_comma_uint(stats.metrics.channels_guild.load(Relaxed))
        );

        let _ = writeln!(
            description,
            "Channels (Private): {}",
            with_comma_uint(stats.metrics.channels_private.load(Relaxed))
        );

        let _ = writeln!(
            description,
            "Emojis: {}",
            with_comma_uint(stats.metrics.emojis.load(Relaxed))
        );

        let _ = writeln!(
            description,
            "Guilds: {}",
            with_comma_uint(stats.metrics.guilds.load(Relaxed))
        );

        let _ = writeln!(
            description,
            "Members: {}",
            with_comma_uint(stats.metrics.members.load(Relaxed))
        );

        let _ = writeln!(
            description,
            "Messages: {}",
            with_comma_uint(stats.metrics.messages.load(Relaxed))
        );

        let _ = writeln!(
            description,
            "Roles: {}",
            with_comma_uint(stats.metrics.roles.load(Relaxed))
        );

        let _ = writeln!(
            description,
            "Unavailable guilds: {}",
            stats.metrics.unavailable_guilds.load(Relaxed)
        );

        let _ = writeln!(description, "Users: {}", stats.metrics.users.load(Relaxed));

        let mut fields = Vec::new();

        let biggest_guilds = stats.biggest_guilds.unwrap();
        let max_name_len = biggest_guilds
            .iter()
            .fold(0, |max, guild| max.max(guild.name.chars().count()));

        let mut guild_value = String::with_capacity(128);
        guild_value.push_str("```\n");

        for guild in biggest_guilds {
            let _ = writeln!(
                guild_value,
                "{:<len$}: {}",
                guild.name,
                with_comma_uint(guild.member_count),
                len = max_name_len
            );
        }

        guild_value.push_str("```");
        fields.push(field!("Biggest guilds".to_owned(), guild_value, false));

        let most_mutuals_users = stats.most_mutuals_users.unwrap();
        let max_name_len = most_mutuals_users
            .iter()
            .fold(0, |max, user| max.max(user.name.chars().count()));

        let mut user_value = String::with_capacity(128);
        user_value.push_str("```\n");

        for user in most_mutuals_users {
            let _ = writeln!(
                user_value,
                "{:<len$}: {}",
                user.name,
                user.mutual_count,
                len = max_name_len
            );
        }

        user_value.push_str("```");
        fields.push(field!("Most mutual guilds".to_owned(), user_value, false));

        Self {
            description,
            fields,
            footer: Footer::new("Boot time"),
            timestamp: start_time,
        }
    }
}

impl EmbedData for CacheEmbed {
    fn into_builder(self) -> EmbedBuilder {
        EmbedBuilder::new()
            .description(self.description)
            .fields(self.fields)
            .footer(self.footer)
            .timestamp(self.timestamp)
    }
}
