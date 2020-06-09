use crate::{Error, MySQL, Osu};

use rand::RngCore;
use rosu::{backend::BeatmapRequest, models::GameMode};
use serenity::prelude::{RwLock, TypeMap};
use std::{collections::VecDeque, path::PathBuf, str::FromStr, sync::Arc};
use tokio::{fs, stream::StreamExt};

pub async fn get_random_filename(
    previous_ids: &mut VecDeque<u32>,
    mode: GameMode,
    path: &PathBuf,
) -> Result<String, Error> {
    let mut files: Vec<String> = fs::read_dir(path)
        .await?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>()
        .await;
    let mut rng = rand::thread_rng();
    let buffer_size = match mode {
        GameMode::STD => 500,
        GameMode::MNA => 100,
        _ => unreachable!(),
    };
    loop {
        let len = files.len();
        let file = files.remove(rng.next_u32() as usize % len);
        let id = u32::from_str(file.split('.').next().unwrap()).unwrap();
        if !previous_ids.contains(&id) {
            previous_ids.push_front(id);
            if previous_ids.len() > buffer_size {
                previous_ids.pop_back();
            }
            return Ok(file);
        }
    }
}

pub async fn get_title_artist(
    mapset_id: u32,
    data: Arc<RwLock<TypeMap>>,
) -> Result<(String, String), Error> {
    let (mut title, artist) = {
        let data = data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        if let Ok(mapset) = mysql.get_beatmapset(mapset_id).await {
            Ok((mapset.title, mapset.artist))
        } else {
            let request = BeatmapRequest::new().mapset_id(mapset_id);
            let osu = data.get::<Osu>().unwrap();
            match request.queue_single(&osu).await {
                Ok(Some(map)) => Ok((map.title, map.artist)),
                _ => Err(Error::Custom(
                    "Could not retrieve map from osu API".to_string(),
                )),
            }
        }
    }?;
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
