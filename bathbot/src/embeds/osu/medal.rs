use std::borrow::Cow;

use bathbot_model::{OsekaiComment, OsekaiMap, OsekaiMedal};
use bathbot_util::{
    constants::{FIELD_VALUE_SIZE, OSU_BASE},
    osu::flag_url,
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder,
};
use rosu_v2::prelude::GameMode;
use time::OffsetDateTime;
use twilight_model::channel::embed::{Embed, EmbedField};

use crate::{commands::osu::MedalAchieved, manager::redis::RedisData};

#[derive(Clone)]
pub struct MedalEmbed {
    achieved: Option<(AuthorBuilder, FooterBuilder, OffsetDateTime)>,
    fields: Vec<EmbedField>,
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
        let mut fields = Vec::with_capacity(7);

        fields![fields { "Description", medal.description, false }];

        if let Some(solution) = medal.solution.filter(|s| !s.is_empty()) {
            let solution = match solution.cow_replace("<br>", "") {
                Cow::Owned(s) => s,
                Cow::Borrowed(_) => solution,
            };

            fields![fields { "Solution", solution, false }];
        }

        let mode_mods = match (medal.restriction, medal.mods) {
            (None, None) => "Any".to_owned(),
            (None, Some(mods)) => format!("Any • {mods}"),
            (Some(mode), None) => format!("{mode} • Any"),
            (Some(mode), Some(mods)) => format!("{mode} • {mods}"),
        };

        fields![fields {
            "Rarity", format!("{:.2}%", medal.rarity), true;
            "Mode • Mods", mode_mods, true;
            "Group", medal.grouping.to_string(), true;
        }];

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

                let m = format!(
                    " - [{title} [{version}]]({OSU_BASE}b/{map_id}) ({vote_sum:+})\n",
                    title = title.cow_escape_markdown(),
                    version = version.cow_escape_markdown()
                );

                if m.len() + map_value.len() + 7 >= FIELD_VALUE_SIZE {
                    map_value.push_str("`...`\n");

                    break;
                } else {
                    map_value += &m;
                }
            }

            map_value.pop();

            fields![fields { format!("Beatmaps: {len}"), map_value, false }];
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

            fields![fields { "Top comment", value, false }];
        }

        let title = medal.name;
        let thumbnail = medal.icon_url;

        let url = format!(
            "https://osekai.net/medals/?medal={}",
            title.cow_replace(' ', "+").cow_replace(',', "%2C")
        );

        let achieved = achieved.map(|achieved| {
            let user = achieved.user;

            let (country_code, username, user_id) = match &user {
                RedisData::Original(user) => {
                    let country_code = user.country_code.as_str();
                    let username = user.username.as_str();
                    let user_id = user.user_id;

                    (country_code, username, user_id)
                }
                RedisData::Archived(user) => {
                    let country_code = user.country_code.as_str();
                    let username = user.username.as_str();
                    let user_id = user.user_id;

                    (country_code, username, user_id)
                }
            };

            let mut author_url = format!("{OSU_BASE}users/{user_id}");

            match medal.restriction {
                None => {}
                Some(GameMode::Osu) => author_url.push_str("/osu"),
                Some(GameMode::Taiko) => author_url.push_str("/taiko"),
                Some(GameMode::Catch) => author_url.push_str("/fruits"),
                Some(GameMode::Mania) => author_url.push_str("/mania"),
            }

            let author = AuthorBuilder::new(username)
                .url(author_url)
                .icon_url(flag_url(country_code));

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

    pub fn minimized(mut self) -> Embed {
        self.fields.truncate(5);

        self.maximized()
    }

    pub fn maximized(self) -> Embed {
        let builder = EmbedBuilder::new()
            .fields(self.fields)
            .thumbnail(self.thumbnail)
            .title(self.title)
            .url(self.url);

        match self.achieved {
            Some((author, footer, timestamp)) => builder
                .author(author)
                .footer(footer)
                .timestamp(timestamp)
                .build(),
            None => builder.build(),
        }
    }
}
