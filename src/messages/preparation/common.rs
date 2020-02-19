#![allow(clippy::too_many_arguments)]

use crate::{
    messages::{AVATAR_URL, HOMEPAGE},
    util::numbers::round,
    Error,
};

use image::{
    imageops::FilterType, png::PNGEncoder, ColorType, DynamicImage, GenericImage, GenericImageView,
};
use itertools::Itertools;
use reqwest::Client;
use rosu::models::{Beatmap, Score, User};
use std::collections::HashMap;
use tokio::runtime::Runtime;

pub struct CommonData {
    pub description: String,
}

impl CommonData {
    pub fn new(
        users: HashMap<u32, User>,
        all_scores: Vec<Vec<Score>>,
        maps: HashMap<u32, Beatmap>,
    ) -> (Self, Vec<u8>) {
        // Flatten scores, sort by beatmap id, then group by beatmap id
        let mut all_scores: Vec<Score> = all_scores.into_iter().flatten().collect();
        all_scores.sort_by(|s1, s2| s1.beatmap_id.unwrap().cmp(&s2.beatmap_id.unwrap()));
        let mut all_scores: HashMap<u32, Vec<Score>> = all_scores
            .into_iter()
            .group_by(|score| score.beatmap_id.unwrap())
            .into_iter()
            .map(|(map_id, scores)| (map_id, scores.collect()))
            .collect();
        // Sort each group by pp value, then take the best 3
        all_scores.iter_mut().for_each(|(_, scores)| {
            scores.sort_by(|s1, s2| s2.pp.unwrap().partial_cmp(&s1.pp.unwrap()).unwrap());
            scores.truncate(3);
        });
        // Consider only the top 10 maps with the highest avg pp among the users
        let mut pp_avg: Vec<(u32, f32)> = all_scores
            .iter()
            .map(|(&map_id, scores)| {
                let sum = scores.iter().fold(0.0, |sum, next| sum + next.pp.unwrap());
                (map_id, sum / scores.len() as f32)
            })
            .collect();
        pp_avg.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let top_map_ids: Vec<u32> = pp_avg.into_iter().take(10).map(|(id, _)| id).collect();
        all_scores.retain(|id, _| top_map_ids.contains(id));
        // Write msg
        let mut description = String::with_capacity(512);
        for (i, map_id) in top_map_ids.iter().enumerate() {
            let map = maps.get(map_id).unwrap();
            description.push_str(&format!(
                "**{idx}.** [{title} [{version}]]({base}b/{id})\n",
                idx = i + 1,
                title = map.title,
                version = map.version,
                base = HOMEPAGE,
                id = map.beatmap_id,
            ));
            let scores = all_scores.get(map_id).unwrap();
            let first_score = scores.get(0).unwrap();
            let first_user = users.get(&first_score.user_id).unwrap();
            let second_score = scores.get(1).unwrap();
            let second_user = users.get(&second_score.user_id).unwrap();
            description.push_str(&format!(
                "- :first_place: `{}`: {}pp :second_place: `{}`: {}pp",
                first_user.username,
                round(first_score.pp.unwrap()),
                second_user.username,
                round(second_score.pp.unwrap())
            ));
            if users.len() > 2 {
                let third_score = scores.get(2).unwrap();
                let third_user = users.get(&third_score.user_id).unwrap();
                description.push_str(&format!(
                    " :third_place: `{}`: {}pp",
                    third_user.username,
                    round(third_score.pp.unwrap())
                ));
            }
            description.push('\n');
        }
        description.pop();
        // Keys have no strict order, hence inconsistent result
        let user_ids: Vec<u32> = users.keys().copied().collect();
        let thumbnail = get_thumbnail(&user_ids).unwrap_or_else(|e| {
            warn!("Error while combining avatars: {}", e);
            Vec::new()
        });
        (Self { description }, thumbnail)
    }
}

fn get_thumbnail(user_ids: &[u32]) -> Result<Vec<u8>, Error> {
    let mut combined = DynamicImage::new_rgba8(128, 128);
    let amount = user_ids.len() as u32;
    let w = 128 / amount;
    let client = Client::new();
    let mut rt = Runtime::new().unwrap();
    for (i, id) in user_ids.iter().enumerate() {
        let url = format!("{}{}", AVATAR_URL, id);
        let res = rt.block_on(async { client.get(&url).send().await?.bytes().await })?;
        let img =
            image::load_from_memory(res.as_ref())?.resize_exact(128, 128, FilterType::Lanczos3);
        let x = i as u32 * 128 / amount;
        for i in 0..w {
            for j in 0..128 {
                let pixel = img.get_pixel(x + i, j);
                combined.put_pixel(x + i, j, pixel);
            }
        }
    }
    let mut png_bytes: Vec<u8> = Vec::with_capacity(16_384); // 2^14 = 128x128
    let png_encoder = PNGEncoder::new(&mut png_bytes);
    png_encoder.encode(&combined.to_bytes(), 128, 128, ColorType::Rgba8)?;
    Ok(png_bytes)
}
