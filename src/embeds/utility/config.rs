use std::fmt::{Display, Write};

use bathbot_psql::model::configs::{ListSize, MinimizedPp, OsuUsername, ScoreSize, UserConfig};
use command_macros::EmbedData;
use rosu_v2::prelude::GameMode;
use twilight_model::{channel::embed::EmbedField, user::User};

use crate::util::builder::AuthorBuilder;

#[derive(EmbedData)]
pub struct ConfigEmbed {
    author: AuthorBuilder,
    fields: Vec<EmbedField>,
    title: &'static str,
}

impl ConfigEmbed {
    pub fn new(author: &User, config: UserConfig<OsuUsername>, twitch: Option<String>) -> Self {
        let author_img = match author.avatar {
            Some(ref hash) if hash.is_animated() => format!(
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

        let author = AuthorBuilder::new(&author.name).icon_url(author_img);
        let title = "Current user configuration:";

        let account_value = format!(
            "```\n\
            osu!: {}\n\
            Twitch: {}\n\
            ```",
            if let Some(ref name) = config.osu {
                name as &dyn Display
            } else {
                &"-" as &dyn Display
            },
            if let Some(name) = twitch.as_ref() {
                name as &dyn Display
            } else {
                &"-" as &dyn Display
            }
        );

        let fields = vec![
            EmbedField {
                inline: true,
                name: "Accounts".to_owned(),
                value: account_value,
            },
            create_field(
                "Retries",
                config.show_retries.unwrap_or(true),
                &[(true, "show"), (false, "hide")],
            ),
            create_field(
                "Minimized PP",
                config.minimized_pp.unwrap_or_default(),
                &[(MinimizedPp::MaxPp, "max pp"), (MinimizedPp::IfFc, "if FC")],
            ),
            create_field(
                "Score embeds",
                config.score_size.unwrap_or_default(),
                &[
                    (ScoreSize::AlwaysMinimized, "always minimized"),
                    (ScoreSize::AlwaysMaximized, "always maximized"),
                    (ScoreSize::InitialMaximized, "initial maximized"),
                ],
            ),
            create_field(
                "List embeds",
                config.list_size.unwrap_or_default(),
                &[
                    (ListSize::Condensed, "condensed"),
                    (ListSize::Detailed, "detailed"),
                    (ListSize::Single, "single"),
                ],
            ),
            create_field(
                "Mode",
                config.mode,
                &[
                    (None, "none"),
                    (Some(GameMode::Osu), "osu"),
                    (Some(GameMode::Taiko), "taiko"),
                    (Some(GameMode::Catch), "catch"),
                    (Some(GameMode::Mania), "mania"),
                ],
            ),
        ];

        Self {
            author,
            fields,
            title,
        }
    }
}

pub(super) fn create_field<T: Eq>(
    name: &'static str,
    val: T,
    options: &[(T, &'static str)],
) -> EmbedField {
    let longest = options.iter().fold(0, |len, (_, text)| len.max(text.len()));
    let capacity = 3 + 1 + options.len() * (longest + 2) + 3;
    let mut value = String::with_capacity(capacity);

    value.push_str("```\n");

    for (option, text) in options {
        let symbol = if &val == option { '>' } else { ' ' };
        let _ = writeln!(value, "{symbol}{text:<longest$}");
    }

    value.push_str("```");

    EmbedField {
        inline: true,
        name: name.to_owned(),
        value,
    }
}
