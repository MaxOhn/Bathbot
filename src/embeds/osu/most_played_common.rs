use crate::{
    custom_client::MostPlayedMap,
    embeds::{osu, EmbedData},
    util::constants::OSU_BASE,
};

use twilight_embed_builder::image_source::ImageSource;
use rosu::models::User;
use std::{collections::HashMap, fmt::Write};

#[derive(Clone)]
pub struct MostPlayedCommonEmbed {
    description: String,
    thumbnail: ImageSource,
}

impl MostPlayedCommonEmbed {
    pub fn new(
        users: &HashMap<u32, User>,
        maps: &[MostPlayedMap],
        users_count: &HashMap<u32, HashMap<u32, u32>>,
        index: usize,
    ) -> Self {
        let mut description = String::with_capacity(512);
        for (i, map) in maps.iter().enumerate() {
            let _ = writeln!(
                description,
                "**{idx}.** [{title} [{version}]]({base}b/{id}) [{stars}]",
                idx = index + i + 1,
                title = map.title,
                version = map.version,
                base = OSU_BASE,
                id = map.beatmap_id,
                stars = osu::get_stars(map.stars),
            );
            let mut top_users: Vec<(u32, u32)> = users_count
                .iter()
                .map(|(user_id, entry)| (*user_id, *entry.get(&map.beatmap_id).unwrap()))
                .collect();
            top_users.sort_by(|a, b| b.1.cmp(&a.1));
            let mut top_users = top_users.into_iter().take(3);
            let (first_name, first_count) = top_users
                .next()
                .map(|(user_id, count)| (&users.get(&user_id).unwrap().username, count))
                .unwrap();
            let (second_name, second_count) = top_users
                .next()
                .map(|(user_id, count)| (&users.get(&user_id).unwrap().username, count))
                .unwrap();
            let _ = write!(
                description,
                "- :first_place: `{}`: **{}** :second_place: `{}`: **{}**",
                first_name, first_count, second_name, second_count
            );
            if let Some((third_id, third_count)) = top_users.next() {
                let third_name = &users.get(&third_id).unwrap().username;
                let _ = write!(
                    description,
                    " :third_place: `{}`: **{}**",
                    third_name, third_count
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

impl EmbedData for MostPlayedCommonEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
}
