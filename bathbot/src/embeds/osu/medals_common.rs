use std::{borrow::Cow, fmt::Write};

use bathbot_macros::EmbedData;
use bathbot_util::{CowUtils, FooterBuilder};
use rosu_v2::prelude::Username;

use crate::{commands::osu::MedalEntryCommon, embeds::attachment, pagination::Pages};

pub struct MedalsCommonUser {
    name: Username,
    winner: usize,
}

impl MedalsCommonUser {
    pub fn new(name: Username, winner: usize) -> Self {
        Self { name, winner }
    }
}

#[derive(EmbedData)]
pub struct MedalsCommonEmbed {
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
    title: &'static str,
}

impl MedalsCommonEmbed {
    pub fn new(
        user1: &MedalsCommonUser,
        user2: &MedalsCommonUser,
        medals: &[MedalEntryCommon],
        pages: &Pages,
    ) -> Self {
        let mut description = String::with_capacity(512);

        for (entry, i) in medals.iter().zip(pages.index() + 1..) {
            let _ = writeln!(
                description,
                "**{i}. [{name}](https://osekai.net/medals/?medal={medal})**",
                name = entry.medal.name,
                medal = entry
                    .medal
                    .name
                    .cow_replace(' ', "+")
                    .cow_replace(',', "%2C"),
            );

            let (timestamp1, timestamp2, first_earlier) = match (entry.achieved1, entry.achieved2) {
                (Some(a1), Some(a2)) => (
                    Some(a1.unix_timestamp()),
                    Some(a2.unix_timestamp()),
                    a1 < a2,
                ),
                (Some(a1), None) => (Some(a1.unix_timestamp()), None, true),
                (None, Some(a2)) => (None, Some(a2.unix_timestamp()), false),
                (None, None) => unreachable!(),
            };

            let _ = writeln!(
                description,
                "- :{medal1}_place: `{name1}`: {timestamp1} \
                :{medal2}_place: `{name2}`: {timestamp2}",
                medal1 = if first_earlier { "first" } else { "second" },
                name1 = user1.name,
                timestamp1 = timestamp(timestamp1),
                medal2 = if first_earlier { "second" } else { "first" },
                name2 = user2.name,
                timestamp2 = timestamp(timestamp2),
            );
        }

        description.pop();

        let footer = format!(
            "🥇 count | {}: {} | {}: {}",
            user1.name, user1.winner, user2.name, user2.winner
        );

        Self {
            description,
            footer: FooterBuilder::new(footer),
            thumbnail: attachment("avatar_fuse.png"),
            title: "Who got which medal first",
        }
    }
}

fn timestamp(timestamp: Option<i64>) -> Cow<'static, str> {
    match timestamp {
        Some(timestamp) => format!("<t:{timestamp}:d>").into(),
        None => "Never".into(),
    }
}
