use crate::{embeds::EmbedData, util::constants::OSU_BASE};

use rosu::model::{Beatmap, User};
use std::{collections::HashMap, fmt::Write};
use twilight_embed_builder::image_source::ImageSource;

pub type MapScores = HashMap<u32, (Beatmap, Vec<(usize, f32)>)>;

pub struct CommonEmbed {
    description: String,
    thumbnail: ImageSource,
}

impl CommonEmbed {
    pub fn new(
        users: &[User],
        map_scores: &MapScores,
        id_pps: &[(u32, f32)],
        index: usize,
    ) -> Self {
        let mut description = String::with_capacity(512);

        for (i, (map_id, _)) in id_pps.iter().enumerate() {
            let (map, scores) = match map_scores.get(map_id) {
                Some(tuple) => tuple,
                None => {
                    warn!("Missing map {} for common embed", map_id);

                    continue;
                }
            };

            let _ = writeln!(
                description,
                "**{idx}.** [{title} [{version}]]({base}b/{id})",
                idx = index + i + 1,
                title = map.title,
                version = map.version,
                base = OSU_BASE,
                id = map.beatmap_id,
            );

            description.push('-');

            for (i, (pos, pp)) in scores.iter().enumerate() {
                let _ = write!(
                    description,
                    " :{medal}_place: `{name}`: {pp:.2}pp",
                    medal = match pos {
                        0 => "first",
                        1 => "second",
                        2 => "third",
                        _ => unreachable!(),
                    },
                    name = users[i].username,
                    pp = pp,
                );
            }

            description.push('\n');
        }

        Self {
            description,
            thumbnail: ImageSource::attachment("avatar_fuse.png").unwrap(),
        }
    }
}

impl EmbedData for CommonEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }

    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
}
