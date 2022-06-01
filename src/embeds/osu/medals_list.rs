use std::fmt::Write;

use command_macros::EmbedData;
use rosu_v2::prelude::User;

use crate::{
    commands::osu::MedalEntryList,
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        osu::flag_url,
        CowUtils,
    },
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
        user: &User,
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
                timestamp = entry.achieved.timestamp(),
                group = entry.medal.grouping,
            );
        }

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer = FooterBuilder::new(format!(
            "Page {page}/{pages} | Acquired {}/{} medals",
            acquired.0, acquired.1
        ));

        let author = AuthorBuilder::new(user.username.as_str())
            .url(format!("{OSU_BASE}u/{}", user.user_id))
            .icon_url(flag_url(user.country_code.as_str()));

        Self {
            author,
            description,
            footer,
            thumbnail: user.avatar_url.to_owned(),
        }
    }
}
