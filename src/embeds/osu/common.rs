use crate::{
    commands::osu::CommonUser,
    embeds::{EmbedData, Footer},
    util::constants::OSU_BASE,
};

use rosu::model::Beatmap;
use std::{collections::HashMap, fmt::Write};
use twilight_embed_builder::image_source::ImageSource;

pub type MapScores = HashMap<u32, (Beatmap, Vec<(usize, f32)>)>;

pub struct CommonEmbed {
    description: String,
    thumbnail: ImageSource,
    footer: Footer,
}

impl CommonEmbed {
    pub fn new(
        users: &[CommonUser],
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
                    name = users[i].name(),
                    pp = pp,
                );
            }

            description.push('\n');
        }

        description.pop();

        let mut footer = String::with_capacity(64);
        footer.push_str("🥇 count");

        for user in users {
            let _ = write!(footer, " | {}: {}", user.name(), user.first_count);
        }

        Self {
            footer: Footer::new(footer),
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

    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
}
