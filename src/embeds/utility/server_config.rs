use std::fmt::Write;

use crate::{
    commands::{osu::ProfileSize, utility::GuildData},
    database::GuildConfig,
    embeds::Author,
};

pub struct ServerConfigEmbed {
    author: Author,
    description: String,
    footer: &'static str,
    title: &'static str,
}

impl ServerConfigEmbed {
    pub fn new(guild: GuildData, config: GuildConfig, authorities: &[String]) -> Self {
        let mut author = Author::new(guild.name);

        if let Some(ref hash) = guild.icon {
            let url = format!(
                "https://cdn.discordapp.com/icons/{}/{}.{}",
                guild.id,
                hash,
                if hash.starts_with("a_") {
                    "gif"
                } else {
                    "webp"
                }
            );

            author = author.icon_url(url);
        }

        let title = "Current server configuration:";

        let mut description = String::with_capacity(256);

        description.push_str("```\nAuthorities: ");

        let mut authorities = authorities.iter();

        if let Some(auth) = authorities.next() {
            let _ = write!(description, "@{}", auth);

            for auth in authorities {
                let _ = write!(description, ", @{}", auth);
            }
        } else {
            description.push_str("None");
        }

        description.push_str("\nPrefixes: ");
        let mut prefixes = config.prefixes.iter();

        if let Some(prefix) = prefixes.next() {
            let _ = write!(description, "`{}`", prefix);

            for prefix in prefixes {
                let _ = write!(description, ", `{}`", prefix);
            }
        }

        description.push_str("\n\nSong commands: | Embeds*:\n");

        let songs = config.with_lyrics();

        if songs {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("enabled       | ");

        let embeds = config.embeds_maximized();

        if embeds {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("minimized\n");

        if songs {
            description.push(' ');
        } else {
            description.push('>');
        }

        description.push_str("disabled      | ");

        if embeds {
            description.push(' ');
        } else {
            description.push('>');
        }

        description.push_str("maximized\n---------------------------\nRetries*: | Profile*:\n");

        let retries = config.show_retries();

        if retries {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("show     | ");

        let profile = config.profile_size.unwrap_or_default();

        if profile == ProfileSize::Compact {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("compact\n");

        if retries {
            description.push(' ');
        } else {
            description.push('>');
        }

        description.push_str("hide     | ");

        if profile == ProfileSize::Medium {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("medium\n          | ");

        if profile == ProfileSize::Full {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("full\n```");

        Self {
            author,
            description,
            footer: "*: Only applies if not set in member's user config",
            title,
        }
    }
}

impl_builder!(ServerConfigEmbed {
    author,
    description,
    footer,
    title,
});
