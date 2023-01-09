use std::{cmp::Ordering, fmt::Write};

use bathbot_macros::EmbedData;
use bathbot_util::{constants::OSU_BASE, CowUtils, IntHasher};
use hashbrown::HashMap;
use rosu_v2::prelude::MostPlayedMap;

use crate::{
    manager::redis::{osu::User, RedisData},
    pagination::Pages,
};

#[derive(EmbedData)]
pub struct MostPlayedCommonEmbed {
    description: String,
}

impl MostPlayedCommonEmbed {
    pub fn new(
        user1: &RedisData<User>,
        user2: &RedisData<User>,
        map_counts: &[(u32, usize)],
        maps: &HashMap<u32, ([usize; 2], MostPlayedMap), IntHasher>,
        pages: &Pages,
    ) -> Self {
        let mut description = String::with_capacity(512);

        let name1 = user1.username();
        let name2 = user2.username();

        for ((map_id, _), i) in map_counts.iter().zip(pages.index + 1..) {
            let ([count1, count2], map) = &maps[map_id];

            let (medal1, medal2) = match count1.cmp(count2) {
                Ordering::Less => ("second", "first"),
                Ordering::Equal => ("first", "first"),
                Ordering::Greater => ("first", "second"),
            };

            let _ = writeln!(
                description,
                "**{i}.** [{title} [{version}]]({OSU_BASE}b/{map_id}) [{stars:.2}★]\n\
                - :{medal1}_place: `{name1}`: **{count1}** :{medal2}_place: `{name2}`: **{count2}**",
                title = map.mapset.title.cow_escape_markdown(),
                version = map.map.version.cow_escape_markdown(),
                stars = map.map.stars,
            );
        }

        description.pop();

        Self { description }
    }
}
