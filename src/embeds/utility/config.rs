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
            GameMode::STD => "osu!".to_owned(),
            GameMode::TKO => "Taiko".to_owned(),
            GameMode::CTB => "CtB".to_owned(),
            GameMode::MNA => "Mania".to_owned(),
        };

        let profile = match config.profile_embed_size {
            ProfileSize::Compact => "Compact".to_owned(),
            ProfileSize::Medium => "Medium".to_owned(),
            ProfileSize::Full => "Full".to_owned(),
        };

        let mut fields = vec![
            field!("Mode".to_owned(), mode, true),
            field!("Profile embed size".to_owned(), profile, true),
            field!(
                "Recent embed maximized".to_owned(),
                config.recent_embed_maximize.to_string(),
                false
            ),
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
