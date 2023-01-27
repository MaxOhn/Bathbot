use std::{cmp::Ordering, collections::HashMap, fmt::Write};

use bathbot_macros::EmbedData;
use bathbot_util::{constants::OSU_BASE, FooterBuilder, IntHasher};
use rosu_v2::prelude::{Beatmap, BeatmapsetCompact};

use crate::{commands::osu::CommonScore, embeds::attachment, pagination::Pages};

#[derive(EmbedData)]
pub struct CommonEmbed {
    description: String,
    thumbnail: String,
    footer: FooterBuilder,
}

impl CommonEmbed {
    pub fn new(
        name1: &str,
        name2: &str,
        map_pps: &[(u32, f32)],
        maps: &HashMap<u32, ([CommonScore; 2], Beatmap, BeatmapsetCompact), IntHasher>,
        wins: [u8; 2],
        pages: &Pages,
    ) -> Self {
        let mut description = String::with_capacity(1024);

        for ((map_id, _), i) in map_pps.iter().zip(pages.index() + 1..) {
            let ([score1, score2], map, mapset) = &maps[map_id];

            let (medal1, medal2) = match score1.cmp(score2) {
                Ordering::Less => ("second", "first"),
                Ordering::Equal => ("first", "first"),
                Ordering::Greater => ("first", "second"),
            };

            let _ = writeln!(
                description,
                "**{i}.** [{title} [{version}]]({OSU_BASE}b/{map_id})\n\
                - :{medal1}_place: `{name1}`: {pp1:.2}pp :{medal2}_place: `{name2}`: {pp2:.2}pp",
                title = mapset.title,
                version = map.version,
                pp1 = score1.pp,
                pp2 = score2.pp,
            );
        }

        description.pop();

        let footer = format!("🥇 count • {name1}: {} • {name2}: {}", wins[0], wins[1]);

        Self {
            footer: FooterBuilder::new(footer),
            description,
            thumbnail: attachment("avatar_fuse.png"),
        }
    }
}
