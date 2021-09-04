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
                "https://cdn.discordapp.com/avatars/{}/{}.gif",
                author.id, hash
            ),
            Some(ref hash) => format!(
                "https://cdn.discordapp.com/avatars/{}/{}.png",
                author.id, hash
            ),
            None => format!(
                "https://cdn.discordapp.com/embed/avatars/{}.png",
                author
                    .discriminator
                    .chars()
                    .last()
                    .unwrap()
                    .to_digit(10)
                    .unwrap()
                    % 5
            ),
        };

        let author = Author::new(&author.name).icon_url(author_img);
        let title = "Current user configuration:";

        let mut description = String::with_capacity(256);

        description.push_str("```\n");

        if let Some(name) = config.name {
            let _ = write!(description, "osu!: {}\n\n", name);
        }

        if let Some(name) = twitch {
            let _ = write!(description, "Twitch: {}", name);
        }

        let profile = config.profile_size.unwrap_or_default();

        description.push_str("Mode:  | Profile: | Embeds:\n");

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

        if config.embeds_maximized {
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

        if config.embeds_maximized {
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

        if config.show_retries {
            description.push('>');
        } else {
            description.push(' ');
        }

        description.push_str("show\n       |          | ");

        if config.show_retries {
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
