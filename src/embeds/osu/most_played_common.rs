use crate::{
    custom_client::MostPlayedMap,
    embeds::{osu, EmbedData},
    util::constants::OSU_BASE,
};

use rosu::model::User;
use std::{collections::HashMap, fmt::Write};
use twilight_embed_builder::image_source::ImageSource;

pub struct MostPlayedCommonEmbed {
    description: String,
    thumbnail: ImageSource,
}

impl MostPlayedCommonEmbed {
    pub fn new(
        users: &[User],
        maps: &[MostPlayedMap],
        users_count: &[HashMap<u32, u32>],
        index: usize,
    ) -> Self {
        let mut description = String::with_capacity(512);

        let mut positions = Vec::with_capacity(users.len());

        for (i, map) in maps.iter().enumerate() {
            let map_id = &map.beatmap_id;

            let _ = writeln!(
                description,
                "**{idx}.** [{title} [{version}]]({base}b/{id}) [{stars}]",
                idx = index + i + 1,
                title = map.title,
                version = map.version,
                base = OSU_BASE,
                id = map_id,
                stars = osu::get_stars(map.stars),
            );

            description.push('-');

            positions.extend(users.iter().map(|_| 0_u8));

            let count_0 = users_count[0][map_id];
            let count_1 = users_count[1][map_id];

            positions[(count_0 > count_1) as usize] += 1;

            if let Some(&count_2) = users_count.get(2).and_then(|counts| counts.get(map_id)) {
                positions[2 * (count_0 > count_2) as usize] += 1;
                positions[1 + (count_1 > count_2) as usize] += 1;
            }

            for (i, (user, pos)) in users.iter().zip(positions.drain(..)).enumerate() {
                let _ = write!(
                    description,
                    " :{medal}_place: `{name}`: **{count}**",
                    medal = match pos {
                        0 => "first",
                        1 => "second",
                        2 => "third",
                        _ => unreachable!(),
                    },
                    name = user.username,
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
