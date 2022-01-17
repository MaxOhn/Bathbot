use crate::{commands::osu::ProfileSize, database::UserConfig, embeds::Author};

use rosu_v2::prelude::GameMode;
use std::fmt::Write;
use twilight_model::user::User;

pub struct ConfigEmbed {
    author: Author,
    description: String,
    title: &'static str,
}

impl ConfigEmbed {
    pub fn new(author: &User, config: UserConfig, twitch: Option<String>) -> Self {
        let author_img = match author.avatar {
            Some(ref hash) if hash.starts_with("a_") => format!(
                "https://cdn.discordapp.com/avatars/{}/{hash}.gif",
                author.id
            ),
            Some(ref hash) => format!(
                "https://cdn.discordapp.com/avatars/{}/{hash}.png",
                author.id
            ),
            None => format!(
                "https://cdn.discordapp.com/embed/avatars/{}.png",
                author.discriminator()
            ),
        };

        let author = Author::new(&author.name).icon_url(author_img);
        let title = "Current user configuration:";

        let mut description = String::with_capacity(256);

        description.push_str("```\nosu!: ");

        if let Some(name) = config.username() {
            let _ = writeln!(description, "{name}");
        } else {
            description.push_str("-\n");
        }

        description.push_str("Twitch: ");

        if let Some(name) = twitch {
            let _ = writeln!(description, "{name}");
        } else {
            description.push_str("-\n");
        }

        let profile = config.profile_size.unwrap_or_default();
        description.push_str("\nMode:  | Profile: | Embeds:\n");

        if config.mode.is_none() {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("none  | ");

        if profile == ProfileSize::Compact {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("compact | ");

        if config.embeds_maximized.unwrap_or(true) {
            description.push(' ');
        } else {
            description.push('>');
        }

        description.push_str("minimized\n");

        if config.mode == Some(GameMode::STD) {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("osu   | ");

        if profile == ProfileSize::Medium {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("medium  | ");

        if config.embeds_maximized.unwrap_or(true) {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("maximized\n");

        if config.mode == Some(GameMode::TKO) {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("taiko | ");

        if profile == ProfileSize::Full {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("full    |-----------\n");

        if config.mode == Some(GameMode::CTB) {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("ctb   |          | Retries:\n");

        if config.mode == Some(GameMode::MNA) {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("mania |          | ");

        if config.show_retries.unwrap_or(true) {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("show\n       |          | ");

        if config.show_retries.unwrap_or(true) {
            description.push(' ');
        } else {
            description.push('>');
        }

        description.push_str("hide\n```");

        Self {
            author,
            description,
            title,
        }
    }
}

impl_builder!(ConfigEmbed {
    author,
    description,
    title,
});
