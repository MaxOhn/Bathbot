use std::fmt::{Display, Write};

use ::time::UtcOffset;
use bathbot_psql::model::configs::{ListSize, OsuUsername, Retries, ScoreData, UserConfig};
use bathbot_util::{AuthorBuilder, EmbedBuilder, FooterBuilder};
use rosu_v2::prelude::GameMode;
use twilight_model::{channel::message::embed::EmbedField, user::User};

use crate::embeds::EmbedData;

pub struct ConfigEmbed {
    author: AuthorBuilder,
    fields: Vec<EmbedField>,
    footer: Option<FooterBuilder>,
    title: &'static str,
}

impl ConfigEmbed {
    pub fn new(
        author: &User,
        config: UserConfig<OsuUsername>,
        twitch: Option<Box<str>>,
        skin_url: Option<String>,
    ) -> Self {
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
            if let Some(ref name) = twitch {
                name as &dyn Display
            } else {
                &"-" as &dyn Display
            },
        );

        let mut fields = vec![
            EmbedField {
                inline: false,
                name: "Accounts".to_owned(),
                value: account_value,
            },
            create_field(
                "Render button",
                config.render_button,
                &[(Some(true), "show"), (Some(false), "hide")],
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
                "Score data",
                config.score_data.unwrap_or(ScoreData::Lazer),
                &[
                    (ScoreData::Stable, "stable"),
                    (ScoreData::Lazer, "lazer"),
                    (
                        ScoreData::LazerWithClassicScoring,
                        "lazer (classic scoring)",
                    ),
                ],
            ),
            create_field(
                "Mode",
                config.mode,
                &[
                    (Some(GameMode::Osu), "osu"),
                    (Some(GameMode::Taiko), "taiko"),
                    (Some(GameMode::Catch), "catch"),
                    (Some(GameMode::Mania), "mania"),
                ],
            ),
            create_field(
                "Retries",
                config.retries.unwrap_or(Retries::ConsiderMods),
                &[
                    (Retries::Hide, "hide"),
                    (Retries::ConsiderMods, "reset on different mods"),
                    (Retries::IgnoreMods, "ignore mods"),
                ],
            ),
        ];

        if let Some(skin_url) = skin_url {
            fields.push(EmbedField {
                inline: false,
                name: "Skin".to_owned(),
                value: skin_url,
            });
        }

        let footer = config
            .timezone
            .map(UtcOffset::whole_hours)
            .map(|tz| format!("Timezone: UTC{tz:+}"))
            .map(FooterBuilder::new);

        Self {
            author,
            fields,
            footer,
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

impl EmbedData for ConfigEmbed {
    #[inline]
    fn build(self) -> EmbedBuilder {
        let mut builder = EmbedBuilder::new()
            .author(self.author)
            .fields(self.fields)
            .title(self.title);

        if let Some(footer) = self.footer {
            builder = builder.footer(footer);
        }

        builder
    }
}
