use chrono::{DateTime, Utc};
use rosu_v2::prelude::GameMode;

use crate::{
    commands::osu::MedalAchieved,
    custom_client::{OsekaiComment, OsekaiMap, OsekaiMedal},
    embeds::{  EmbedData, EmbedFields, },
    util::{
        constants::{FIELD_VALUE_SIZE, OSU_BASE},
        osu::flag_url,
        CowUtils, builder::{FooterBuilder, AuthorBuilder, EmbedBuilder},
    },
};

#[derive(Clone)]
pub struct MedalEmbed {
    achieved: Option<(AuthorBuilder, FooterBuilder, DateTime<Utc>)>,
    fields: EmbedFields,
    thumbnail: String,
    title: String,
    url: String,
}

impl MedalEmbed {
    pub fn new(
        medal: OsekaiMedal,
        achieved: Option<MedalAchieved<'_>>,
        maps: Vec<OsekaiMap>,
        comment: Option<OsekaiComment>,
    ) -> Self {
        let mode = medal
            .restriction
            .map_or_else(|| "Any".to_owned(), |mode| mode.to_string());

        let mods = medal
            .mods
            .map_or_else(|| "Any".to_owned(), |mods| mods.to_string());

        let mut fields = Vec::with_capacity(7);
        fields.push(field!("Description", medal.description, false));

        if let Some(solution) = medal.solution {
            fields.push(field!("Solution", solution, false));
        }

        fields.push(field!("Mode", mode, true));
        fields.push(field!("Mods", mods, true));
        fields.push(field!("Group", medal.grouping, true));

        if !maps.is_empty() {
            let len = maps.len();
            let mut map_value = String::with_capacity(256);

            for map in maps {
                let OsekaiMap {
                    title,
                    version,
                    map_id,
                    vote_sum,
                    ..
                } = map;

                let m = format!(" - [{title} [{version}]]({OSU_BASE}b/{map_id}) ({vote_sum:+})\n",);

                if m.len() + map_value.len() + 7 >= FIELD_VALUE_SIZE {
                    map_value.push_str("`...`\n");

                    break;
                } else {
                    map_value += &m;
                }
            }

            map_value.pop();
            fields.push(field!(format!("Beatmaps: {len}"), map_value, false));
        }

        if let Some(comment) = comment {
            let OsekaiComment {
                content,
                username,
                vote_sum,
                ..
            } = comment;

            let value = format!(
                "```\n\
                {content}\n    \
                - {username} [{vote_sum:+}]\n\
                ```",
                content = content.trim(),
            );

            fields.push(field!("Top comment", value, false));
        }

        let title = medal.name;
        let thumbnail = medal.icon_url;

        let url = format!(
            "https://osekai.net/medals/?medal={}",
            title.cow_replace(' ', "+").cow_replace(',', "%2C")
        );

        let achieved = achieved.map(|achieved| {
            let user = achieved.user;

            let mut author_url = format!("{OSU_BASE}users/{}", user.user_id);

            match medal.restriction {
                None => {}
                Some(GameMode::STD) => author_url.push_str("/osu"),
                Some(GameMode::TKO) => author_url.push_str("/taiko"),
                Some(GameMode::CTB) => author_url.push_str("/fruits"),
                Some(GameMode::MNA) => author_url.push_str("/mania"),
            }

            let author = AuthorBuilder::new(user.username.as_str())
                .url(author_url)
                .icon_url(flag_url(user.country_code.as_str()));

            let footer = FooterBuilder::new(format!(
                "Medal {}/{} | Achieved",
                achieved.index, achieved.medal_count
            ));

            (author, footer, achieved.achieved_at)
        });

        Self {
            achieved,
            fields,
            thumbnail,
            title,
            url,
        }
    }

    pub fn minimized(mut self) -> EmbedBuilder {
        self.fields.truncate(5);

        self.into_builder()
    }

    pub fn maximized(self) -> EmbedBuilder {
        self.into_builder()
    }
}

impl EmbedData for MedalEmbed {
    fn into_builder(self) -> EmbedBuilder {
        let builder = EmbedBuilder::new()
            .fields(self.fields)
            .thumbnail(self.thumbnail)
            .title(self.title)
            .url(self.url);

        match self.achieved {
            Some((author, footer, timestamp)) => {
                builder.author(author).footer(footer).timestamp(timestamp)
            }
            None => builder,
        }
    }
}
