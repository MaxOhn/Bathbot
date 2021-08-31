use crate::{
    database::OsuMedal,
    embeds::{attachment, Footer},
    util::CowUtils,
};

use hashbrown::HashMap;
use std::{borrow::Cow, fmt::Write};

pub struct MedalsCommonUser {
    name: String,
    medals: HashMap<u32, i64>,
    winner: usize,
}

impl MedalsCommonUser {
    pub fn new(name: String, medals: HashMap<u32, i64>, winner: usize) -> Self {
        Self {
            name,
            medals,
            winner,
        }
    }
}

pub struct MedalsCommonEmbed {
    description: String,
    footer: Footer,
    thumbnail: String,
    title: &'static str,
}

impl MedalsCommonEmbed {
    pub fn new(
        user1: &MedalsCommonUser,
        user2: &MedalsCommonUser,
        medals: &[OsuMedal],
        index: usize,
    ) -> Self {
        let mut description = String::with_capacity(512);

        for (i, medal) in medals.iter().enumerate() {
            let _ = writeln!(
                description,
                "**{idx}. [{name}](https://osekai.net/medals/?medal={medal})**",
                idx = index + i + 1,
                name = medal.name,
                medal = medal.name.cow_replace(' ', "+").cow_replace(',', "%2C"),
            );

            let (timestamp1, timestamp2, first_earlier) = match (
                user1.medals.get(&medal.medal_id),
                user2.medals.get(&medal.medal_id),
            ) {
                (Some(date1), Some(date2)) => (Some(date1), Some(date2), date1 < date2),
                (Some(date), None) => (Some(date), None, true),
                (None, Some(date)) => (None, Some(date), false),
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
            footer: Footer::new(footer),
            thumbnail: attachment("avatar_fuse.png"),
            title: "Who got which medal first",
        }
    }
}

fn timestamp(timestamp: Option<&i64>) -> Cow<'static, str> {
    match timestamp {
        Some(timestamp) => format!("<t:{}:d>", timestamp).into(),
        None => "Never".into(),
    }
}

impl_builder!(MedalsCommonEmbed {
    description,
    footer,
    thumbnail,
    title,
});
