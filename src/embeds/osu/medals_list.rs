use std::fmt::Write;

use rosu_v2::prelude::User;

use crate::{
    commands::osu::MedalEntryList,
    embeds::{Author, Footer},
    util::{constants::OSU_BASE, osu::flag_url, CowUtils},
};

pub struct MedalsListEmbed {
    author: Author,
    description: String,
    footer: Footer,
    thumbnail: String,
}

impl MedalsListEmbed {
    pub fn new(
        user: &User,
        medals: &[MedalEntryList],
        acquired: (usize, usize),
        pages: (usize, usize),
    ) -> Self {
        let mut description = String::with_capacity(1024);
        let offset = (pages.0 - 1) * 10;

        for (entry, i) in medals.iter().zip(offset + 1..) {
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

        let footer = Footer::new(format!(
            "Page {}/{} | Acquired {}/{} medals",
            pages.0, pages.1, acquired.0, acquired.1
        ));

        let author = Author::new(user.username.as_str())
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

impl_builder!(MedalsListEmbed {
    author,
    description,
    footer,
    thumbnail,
});
