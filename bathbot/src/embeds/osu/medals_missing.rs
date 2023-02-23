use std::fmt::Write;

use bathbot_macros::EmbedData;
use bathbot_util::{constants::OSU_BASE, osu::flag_url, AuthorBuilder, CowUtils, FooterBuilder};

use crate::{
    commands::osu::MedalType,
    manager::redis::{osu::User, RedisData},
    pagination::Pages,
};

#[derive(EmbedData)]
pub struct MedalsMissingEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
    title: &'static str,
}

impl MedalsMissingEmbed {
    pub fn new(
        user: &RedisData<User>,
        medals: &[MedalType],
        medal_count: (usize, usize),
        includes_last: bool,
        pages: &Pages,
    ) -> Self {
        let mut description = String::new();

        for (i, medal) in medals.iter().enumerate() {
            match medal {
                MedalType::Group(g) => {
                    let _ = writeln!(description, "__**{g}:**__");

                    if let Some(MedalType::Group(_)) = medals.get(i + 1) {
                        description.push_str("All medals acquired\n");
                    } else if i == medals.len() - 1 && includes_last {
                        description.push_str("All medals acquired");
                    }
                }
                MedalType::Medal(m) => {
                    let _ = writeln!(
                        description,
                        "- [{}](https://osekai.net/medals/?medal={} \"Rarity: {:.2}%\")",
                        m.name,
                        m.name.cow_replace(' ', "+"),
                        m.rarity,
                    );
                }
            }
        }

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer = FooterBuilder::new(format!(
            "Page {page}/{pages} | Missing {}/{} medals",
            medal_count.0, medal_count.1
        ));

        let (country_code, username, user_id, avatar_url) = match user {
            RedisData::Original(user) => {
                let country_code = user.country_code.as_str();
                let username = user.username.as_str();
                let user_id = user.user_id;
                let avatar_url = user.avatar_url.as_str();

                (country_code, username, user_id, avatar_url)
            }
            RedisData::Archived(user) => {
                let country_code = user.country_code.as_str();
                let username = user.username.as_str();
                let user_id = user.user_id;
                let avatar_url = user.avatar_url.as_str();

                (country_code, username, user_id, avatar_url)
            }
        };

        let author = AuthorBuilder::new(username)
            .url(format!("{OSU_BASE}u/{user_id}"))
            .icon_url(flag_url(country_code));

        Self {
            author,
            description,
            footer,
            thumbnail: avatar_url.to_owned(),
            title: "Missing medals",
        }
    }
}
