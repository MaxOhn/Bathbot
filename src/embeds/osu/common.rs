use crate::{embeds::EmbedData, util::constants::OSU_BASE};

use rosu::models::{Beatmap, Score, User};
use std::{collections::HashMap, fmt::Write};
use twilight_embed_builder::image_source::ImageSource;

#[derive(Clone)]
pub struct CommonEmbed {
    description: String,
    thumbnail: ImageSource,
}

impl CommonEmbed {
    pub fn new(
        users: &HashMap<u32, User>,
        scores: &HashMap<u32, Vec<Score>>,
        maps: &HashMap<u32, Beatmap>,
        id_pps: &[(u32, f32)],
        index: usize,
    ) -> Self {
        let mut description = String::with_capacity(512);
        for (i, (map_id, _)) in id_pps.iter().enumerate() {
            let map = maps.get(map_id).unwrap();
            let _ = writeln!(
                description,
                "**{idx}.** [{title} [{version}]]({base}b/{id})",
                idx = index + i + 1,
                title = map.title,
                version = map.version,
                base = OSU_BASE,
                id = map.beatmap_id,
            );
            let scores = scores.get(map_id).unwrap();
            let first_score = scores.get(0).unwrap();
            let first_user = users.get(&first_score.user_id).unwrap();
            let second_score = scores.get(1).unwrap();
            let second_user = users.get(&second_score.user_id).unwrap();
            let _ = write!(
                description,
                "- :first_place: `{}`: {:.2}pp :second_place: `{}`: {:.2}pp",
                first_user.username,
                first_score.pp.unwrap(),
                second_user.username,
                second_score.pp.unwrap()
            );
            if users.len() > 2 {
                let third_score = scores.get(2).unwrap();
                let third_user = users.get(&third_score.user_id).unwrap();
                let _ = write!(
                    description,
                    " :third_place: `{}`: {:.2}pp",
                    third_user.username,
                    third_score.pp.unwrap()
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
