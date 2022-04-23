use crate::{embeds::osu, util::constants::OSU_BASE};

use hashbrown::HashMap;
use rosu_v2::prelude::MostPlayedMap;
use std::{cmp::Ordering, fmt::Write};

pub struct MostPlayedCommonEmbed {
    description: String,
}

impl MostPlayedCommonEmbed {
    pub fn new(
        name1: &str,
        name2: &str,
        map_counts: &[(u32, usize)],
        maps: &HashMap<u32, ([usize; 2], MostPlayedMap)>,
        index: usize,
    ) -> Self {
        let mut description = String::with_capacity(512);

        for ((map_id, _), i) in map_counts.iter().zip(1..) {
            let ([count1, count2], map) = maps.get(map_id).unwrap();

            let (medal1, medal2) = match count1.cmp(&count2) {
                Ordering::Less => ("second", "first"),
                Ordering::Equal => ("first", "first"),
                Ordering::Greater => ("first", "second"),
            };

            let _ = writeln!(
                description,
                "**{idx}.** [{title} [{version}]]({OSU_BASE}b/{map_id}) [{stars}]\n\
                - :{medal1}_place: `{name1}`: **{count1}** :{medal2}_place: `{name2}`: **{count2}**",
                idx = index + i,
                title = map.mapset.title,
                version = map.map.version,
                stars = osu::get_stars(map.map.stars),
            );
        }

        description.pop();

        Self { description }
    }
}

impl_builder!(MostPlayedCommonEmbed { description });
