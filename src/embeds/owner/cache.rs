use crate::{
    embeds::{EmbedData, Footer},
    util::numbers::with_comma_u64,
    Context,
};

use chrono::{DateTime, Utc};
use itertools::Itertools;
use std::fmt::Write;

pub struct CacheEmbed {
    description: String,
    footer: Footer,
    timestamp: DateTime<Utc>,
    fields: Vec<(String, String, bool)>,
}

impl CacheEmbed {
    pub fn new(ctx: &Context) -> Self {
        let stats = &ctx.cache.stats;
        let events = &stats.event_counts;

        let mut description = String::with_capacity(256);

        let _ = writeln!(
            description,
            "Loaded guilds: {}",
            stats.guild_counts.loaded.get()
        );
        let _ = writeln!(
            description,
            "Partial guilds: {}",
            stats.guild_counts.partial.get()
        );
        let _ = writeln!(
            description,
            "Outage guilds: {}\n",
            stats.guild_counts.outage.get()
        );

        let _ = writeln!(description, "Guild create: {}", events.guild_create.get());
        let _ = writeln!(description, "Guild delete: {}", events.guild_delete.get());
        let _ = writeln!(description, "Guild update: {}\n", events.guild_update.get());

        let _ = writeln!(
            description,
            "User msgs: {}",
            stats.message_counts.user_messages.get()
        );
        let _ = writeln!(
            description,
            "Bot msgs: {}",
            stats.message_counts.other_bot_messages.get()
        );
        let _ = writeln!(
            description,
            "Own msgs: {}\n",
            stats.message_counts.own_messages.get()
        );

        let _ = writeln!(
            description,
            "Users total: {}",
            stats.user_counts.total.get()
        );
        let _ = writeln!(
            description,
            "Users unique: {}",
            stats.user_counts.unique.get()
        );
        let _ = writeln!(description, "User updates: {}\n", events.user_update.get());

        let _ = writeln!(description, "Member add: {}", events.member_add.get());
        let _ = writeln!(description, "Member remove: {}", events.member_remove.get());
        let _ = writeln!(description, "Member update: {}", events.member_update.get());
        let _ = writeln!(description, "Member chunk: {}\n", events.member_chunk.get());

        let _ = writeln!(
            description,
            "Message create: {}",
            events.message_create.get()
        );
        let _ = writeln!(
            description,
            "Message delete: {}",
            events.message_delete.get()
        );
        let _ = writeln!(
            description,
            "Message delete bulk: {}",
            events.message_delete_bulk.get()
        );
        let _ = writeln!(
            description,
            "Message update: {}\n",
            events.message_update.get()
        );

        let _ = writeln!(
            description,
            "Unvailable guilds: {}",
            events.unavailable_guild.get()
        );

        let mut fields = Vec::new();

        let biggest_guilds: Vec<_> = ctx
            .cache
            .guilds
            .iter()
            .map(|guard| (guard.value().members.len(), guard.value().name.clone()))
            .sorted_by(|(a, _), (b, _)| b.cmp(&a))
            .take(15)
            .collect();

        let max_name_len = biggest_guilds
            .iter()
            .fold(0, |max, (_, guild)| max.max(guild.chars().count()));

        let mut guild_value = String::with_capacity(128);
        guild_value.push_str("```\n");
        for (members, name) in biggest_guilds {
            let _ = writeln!(
                guild_value,
                "{:<len$}: {}",
                name,
                members,
                len = max_name_len
            );
        }
        guild_value.push_str("```");
        fields.push(("Biggest guilds".to_owned(), guild_value, false));

        fields.push((
            "Users".to_owned(),
            with_comma_u64(ctx.cache.users.len() as u64),
            true,
        ));
        fields.push((
            "Guild channels".to_owned(),
            with_comma_u64(ctx.cache.guild_channels.len() as u64),
            true,
        ));
        fields.push((
            "Private channels".to_owned(),
            with_comma_u64(ctx.cache.private_channels.len() as u64),
            true,
        ));

        Self {
            description,
            footer: Footer::new("Boot time"),
            timestamp: ctx.cache.stats.start_time,
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
