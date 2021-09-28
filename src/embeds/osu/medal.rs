use crate::{
    commands::osu::MedalAchieved,
    custom_client::{OsekaiComment, OsekaiMap, OsekaiMedal},
    embeds::{Author, EmbedBuilder, EmbedData, EmbedFields, Footer},
    util::{
        constants::{FIELD_VALUE_SIZE, OSU_BASE},
        osu::flag_url,
        CowUtils,
    },
};

use chrono::{DateTime, Utc};
use std::fmt::Write;

#[derive(Clone)]
pub struct MedalEmbed {
    achieved: Option<(Author, Footer, DateTime<Utc>)>,
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
        comments: Vec<OsekaiComment>,
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
                let m = format!(
                    " - [{} [{}]]({}b/{}) (+{})\n",
                    map.title, map.version, OSU_BASE, map.map_id, map.vote_sum
                );

                if m.len() + map_value.len() + 7 >= FIELD_VALUE_SIZE {
                    map_value.push_str("`...`\n");

                    break;
                } else {
                    map_value += &m;
                }
            }

            map_value.pop();
            fields.push(field!(format!("Beatmaps: {}", len), map_value, false));
        }

        if !comments.is_empty() {
            let mut comment_value = String::with_capacity(256);

            let comment_iter = comments
                .into_iter()
                .filter(|comment| comment.parent_id == 0);

            for comment in comment_iter {
                let mut c =
                    String::with_capacity(16 + comment.content.len() + comment.username.len());

                c.push_str("```\n");
                c.push_str(comment.content.as_str());
                let _ = writeln!(c, "\n    - {} [{:+}]", comment.username, comment.vote_sum);
                c.push_str("```\n");

                if c.len() + comment_value.len() < FIELD_VALUE_SIZE {
                    comment_value += &c;
                }
            }

            comment_value.pop();

            if !comment_value.is_empty() {
                fields.push(field!("Comments".to_owned(), comment_value, false));
            }
        }

        let title = medal.name;
        let thumbnail = medal.icon_url;

        let url = format!(
            "https://osekai.net/medals/?medal={}",
            title.cow_replace(' ', "+").cow_replace(',', "%2C")
        );

        let achieved = achieved.map(|achieved| {
            let user = achieved.user;

            let author = Author::new(&user.username)
                .url(format!("{}u/{}", OSU_BASE, user.user_id))
                .icon_url(flag_url(user.country_code.as_str()));

            let footer = Footer::new(format!(
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
