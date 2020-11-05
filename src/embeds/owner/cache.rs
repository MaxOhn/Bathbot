use crate::{
    embeds::{EmbedData, Footer},
    util::numbers::with_comma_u64,
};

use chrono::{DateTime, Utc};
use std::{fmt::Write, sync::atomic::Ordering::Relaxed};
use twilight_cache_inmemory::CacheStats;

pub struct CacheEmbed {
    description: String,
    footer: Footer,
    timestamp: DateTime<Utc>,
    fields: Vec<(String, String, bool)>,
}

impl CacheEmbed {
    pub fn new(stats: CacheStats, start_time: DateTime<Utc>) -> Self {
        let mut description = String::with_capacity(256);

        let _ = writeln!(
            description,
            "Channels (Guilds): {}",
            with_comma_u64(stats.metrics.channels_guild.load(Relaxed) as u64)
        );
        let _ = writeln!(
            description,
            "Channels (Private): {}",
            with_comma_u64(stats.metrics.channels_private.load(Relaxed) as u64)
        );
        let _ = writeln!(
            description,
            "Emojis: {}",
            with_comma_u64(stats.metrics.emojis.load(Relaxed) as u64)
        );
        let _ = writeln!(
            description,
            "Guilds: {}",
            with_comma_u64(stats.metrics.guilds.load(Relaxed) as u64)
        );
        let _ = writeln!(
            description,
            "Members: {}",
            with_comma_u64(stats.metrics.members.load(Relaxed) as u64)
        );
        let _ = writeln!(
            description,
            "Messages: {}",
            with_comma_u64(stats.metrics.messages.load(Relaxed) as u64)
        );
        let _ = writeln!(
            description,
            "Roles: {}",
            with_comma_u64(stats.metrics.roles.load(Relaxed) as u64)
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
                with_comma_u64(guild.member_count as u64),
                len = max_name_len
            );
        }
        guild_value.push_str("```");
        fields.push(("Biggest guilds".to_owned(), guild_value, false));

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
        fields.push(("Most mutual guilds".to_owned(), user_value, false));

        Self {
            description,
            footer: Footer::new("Boot time"),
            timestamp: start_time,
            fields,
        }
    }
}

impl EmbedData for CacheEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn timestamp(&self) -> Option<&DateTime<Utc>> {
        Some(&self.timestamp)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
}
