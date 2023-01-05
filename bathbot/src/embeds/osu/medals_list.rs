use std::fmt::Write;

use bathbot_macros::EmbedData;
use bathbot_util::{constants::OSU_BASE, osu::flag_url, AuthorBuilder, CowUtils, FooterBuilder};

use crate::{
    commands::osu::MedalEntryList,
    manager::redis::{osu::User, RedisData},
    pagination::Pages,
};

#[derive(EmbedData)]
pub struct MedalsListEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
}

impl MedalsListEmbed {
    pub fn new(
        user: &RedisData<User>,
        medals: &[MedalEntryList],
        acquired: (usize, usize),
        pages: &Pages,
    ) -> Self {
        let mut description = String::with_capacity(1024);

        for (entry, i) in medals.iter().zip(pages.index + 1..) {
            let _ = writeln!(
                description,
                "**{i}. [{medal}](https://osekai.net/medals/?medal={url_name})**\n\
                • `{rarity:>5.2}%` • <t:{timestamp}:d> • {group}",
                medal = entry.medal.name,
                url_name = entry.medal.name.cow_replace(' ', "+"),
                rarity = entry.rarity,
                timestamp = entry.achieved.unix_timestamp(),
                group = entry.medal.grouping,
            );
        }

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer = FooterBuilder::new(format!(
            "Page {page}/{pages} | Acquired {}/{} medals",
            acquired.0, acquired.1
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
        }
    }
}
