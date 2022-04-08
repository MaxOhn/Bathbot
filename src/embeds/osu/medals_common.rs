use crate::{
    commands::osu::MedalEntryCommon,
    embeds::{attachment, },
    util::{CowUtils, builder::FooterBuilder},
};

use rosu_v2::prelude::Username;
use std::{borrow::Cow, fmt::Write};

pub struct MedalsCommonUser {
    name: Username,
    winner: usize,
}

impl MedalsCommonUser {
    pub fn new(name: Username, winner: usize) -> Self {
        Self { name, winner }
    }
}

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
        index: usize,
    ) -> Self {
        let mut description = String::with_capacity(512);

        for (i, entry) in medals.iter().enumerate() {
            let _ = writeln!(
                description,
                "**{idx}. [{name}](https://osekai.net/medals/?medal={medal})**",
                idx = index + i + 1,
                name = entry.medal.name,
                medal = entry
                    .medal
                    .name
                    .cow_replace(' ', "+")
                    .cow_replace(',', "%2C"),
            );

            let (timestamp1, timestamp2, first_earlier) = match (entry.achieved1, entry.achieved2) {
                (Some(a1), Some(a2)) => (Some(a1.timestamp()), Some(a2.timestamp()), a1 < a2),
                (Some(a1), None) => (Some(a1.timestamp()), None, true),
                (None, Some(a2)) => (None, Some(a2.timestamp()), false),
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

impl_builder!(MedalsCommonEmbed {
    description,
    footer,
    thumbnail,
    title,
});
