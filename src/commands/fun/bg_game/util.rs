use crate::{database::MapsetTagWrapper, MySQL, Osu};

use failure::Error;
use rand::RngCore;
use rosu::backend::BeatmapRequest;
use serenity::prelude::{RwLock, TypeMap};
use std::collections::VecDeque;

pub async fn get_random_mapset<'m>(
    mapsets: &'m [MapsetTagWrapper],
    previous_ids: &mut VecDeque<u32>,
) -> Result<&'m MapsetTagWrapper, Error> {
    let mut rng = rand::thread_rng();
    let buffer_size = mapsets.len() / 2;
    loop {
        let random_index = rng.next_u32() as usize % mapsets.len();
        let mapset = &mapsets[random_index];
        if !previous_ids.contains(&mapset.mapset_id) {
            previous_ids.push_front(mapset.mapset_id);
            if previous_ids.len() > buffer_size {
                previous_ids.pop_back();
            }
            return Ok(mapset);
        }
    }
}

pub async fn get_title_artist(
    mapset_id: u32,
    data: &RwLock<TypeMap>,
) -> Result<(String, String), Error> {
    let (mut title, artist) = {
        let data = data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        if let Ok(mapset) = mysql.get_beatmapset(mapset_id).await {
            (mapset.title, mapset.artist)
        } else {
            let request = BeatmapRequest::new().mapset_id(mapset_id);
            let osu = data.get::<Osu>().unwrap();
            match request.queue_single(&osu).await {
                Ok(Some(map)) => (map.title, map.artist),
                _ => bail!("Could not retrieve map from osu API"),
            }
        }
    };
    if title.contains('(') && title.contains(')') {
        let idx_open = title.find('(').unwrap();
        let idx_close = title.find(')').unwrap();
        title.replace_range(idx_open..=idx_close, "");
    }
    if let Some(idx) = title.find("feat.").or_else(|| title.find("ft.")) {
        title.truncate(idx);
    }
    title = title.trim().to_string().to_lowercase();
    Ok((title, artist.to_lowercase()))
}

pub fn similarity(word_a: &str, word_b: &str) -> f32 {
    let len = word_a.chars().count().max(word_b.chars().count());
    let dist = levenshtein_distance(word_a, word_b);
    (len - dist) as f32 / len as f32
}

fn levenshtein_distance(word_a: &str, word_b: &str) -> usize {
    let (word_a, word_b) = if word_a.len() > word_b.len() {
        (word_b, word_a)
    } else {
        (word_a, word_b)
    };
    let mut costs: Vec<usize> = (0..=word_b.len()).collect();
    for (i, a) in (1..=word_a.len()).zip(word_a.chars()) {
        let mut last_val = i;
        for (j, b) in (1..=word_b.len()).zip(word_b.chars()) {
            let new_val = if a == b {
                costs[j - 1]
            } else {
                costs[j - 1].min(last_val).min(costs[j]) + 1
            };
            costs[j - 1] = last_val;
            last_val = new_val;
        }
        costs[word_b.len()] = last_val;
    }
    *costs.last().unwrap()
}
