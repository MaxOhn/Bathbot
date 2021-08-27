use crate::{
    commands::osu::ProfileSize,
    database::UserConfig,
    embeds::{Author, EmbedFields},
};

use rosu_v2::prelude::GameMode;
use twilight_model::user::User;

pub struct ConfigEmbed {
    author: Author,
    fields: EmbedFields,
    title: &'static str,
}

impl ConfigEmbed {
    pub fn new(author: &User, config: UserConfig) -> Self {
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

        let mode = match config.mode {
            Some(GameMode::STD) => "osu!",
            Some(GameMode::TKO) => "Taiko",
            Some(GameMode::CTB) => "CtB",
            Some(GameMode::MNA) => "Mania",
            None => "None",
        };

        let profile = match config.profile_embed_size.unwrap_or_default() {
            ProfileSize::Compact => "Compact",
            ProfileSize::Medium => "Medium",
            ProfileSize::Full => "Full",
        };

        let recent = match config.recent_embed_maximize {
            true => "Maximized",
            false => "Minimized",
        };

        let mut fields = vec![
            field!("Mode".to_owned(), mode.to_owned(), true),
            field!("Profile embed size".to_owned(), profile.to_owned(), true),
            field!("Initial embed size".to_owned(), recent.to_owned(), false),
        ];

        if let Some(name) = config.name {
            fields.insert(0, field!("Username".to_owned(), name.into_string(), true));
        }

        Self {
            author,
            fields,
            title,
        }
    }
}

impl_builder!(ConfigEmbed {
    author,
    fields,
    title,
});
