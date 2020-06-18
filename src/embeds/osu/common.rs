use crate::{
    embeds::EmbedData,
    util::{globals::HOMEPAGE, numbers::round},
};

use rosu::models::{Beatmap, Score, User};
use std::{collections::HashMap, fmt::Write};

#[derive(Clone)]
pub struct CommonEmbed {
    description: String,
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
                base = HOMEPAGE,
                id = map.beatmap_id,
            );
            let scores = scores.get(map_id).unwrap();
            let first_score = scores.get(0).unwrap();
            let first_user = users.get(&first_score.user_id).unwrap();
            let second_score = scores.get(1).unwrap();
            let second_user = users.get(&second_score.user_id).unwrap();
            let _ = write!(
                description,
                "- :first_place: `{}`: {}pp :second_place: `{}`: {}pp",
                first_user.username,
                round(first_score.pp.unwrap()),
                second_user.username,
                round(second_score.pp.unwrap())
            );
            if users.len() > 2 {
                let third_score = scores.get(2).unwrap();
                let third_user = users.get(&third_score.user_id).unwrap();
                let _ = write!(
                    description,
                    " :third_place: `{}`: {}pp",
                    third_user.username,
                    round(third_score.pp.unwrap())
                );
            }
            description.push('\n');
        }
        Self { description }
    }
}

impl EmbedData for CommonEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
}
