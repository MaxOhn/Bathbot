use crate::{
    embeds::{osu, EmbedData},
    util::constants::OSU_BASE,
};

use hashbrown::HashMap;
use rosu_v2::prelude::MostPlayedMap;
use std::fmt::Write;
use twilight_embed_builder::image_source::ImageSource;

pub struct MostPlayedCommonEmbed {
    description: String,
    thumbnail: ImageSource,
}

impl MostPlayedCommonEmbed {
    pub fn new(
        names: &[String],
        maps: &[MostPlayedMap],
        users_count: &[HashMap<u32, usize>],
        index: usize,
    ) -> Self {
        let mut description = String::with_capacity(512);
        let mut positions = Vec::with_capacity(names.len());

        for (i, map) in maps.iter().enumerate() {
            let map_id = &map.map.map_id;

            let _ = writeln!(
                description,
                "**{idx}.** [{title} [{version}]]({base}b/{id}) [{stars}]",
                idx = index + i + 1,
                title = map.mapset.title,
                version = map.map.version,
                base = OSU_BASE,
                id = map_id,
                stars = osu::get_stars(map.map.stars),
            );

            description.push('-');

            positions.extend(names.iter().map(|_| 0_u8));

            let count_0 = users_count[0][map_id];
            let count_1 = users_count[1][map_id];

            positions[(count_0 > count_1) as usize] += 1;

            if let Some(&count_2) = users_count.get(2).and_then(|counts| counts.get(map_id)) {
                positions[2 * (count_0 > count_2) as usize] += 1;
                positions[1 + (count_1 > count_2) as usize] += 1;
            }

            for (i, (name, pos)) in names.iter().zip(positions.drain(..)).enumerate() {
                let _ = write!(
                    description,
                    " :{medal}_place: `{name}`: **{count}**",
                    medal = match pos {
                        0 => "first",
                        1 => "second",
                        2 => "third",
                        _ => unreachable!(),
                    },
                    name = name,
                    count = users_count[i][map_id],
                );
            }

            description.push('\n');
        }

        description.pop();

        Self {
            description,
            thumbnail: ImageSource::attachment("avatar_fuse.png").unwrap(),
        }
    }
}

impl EmbedData for MostPlayedCommonEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }

    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
}
