use crate::{util::Matrix, Error, MySQL, Osu};

use rand::RngCore;
use rayon::prelude::*;
use rosu::backend::BeatmapRequest;
use serenity::prelude::{RwLock, ShareMap};
use std::{collections::VecDeque, fs, path::PathBuf, str::FromStr, sync::Arc};
use tokio::runtime::Runtime;

pub fn get_random_filename(
    previous_ids: &mut VecDeque<u32>,
    path: &PathBuf,
) -> Result<String, Error> {
    let mut files: Vec<String> = fs::read_dir(path)?
        .into_iter()
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|entry| {
            // consider only files and no directories
            if let Ok(entry) = entry {
                if let Ok(true) = entry.file_type().map(|ft| ft.is_file()) {
                    return Some(entry.file_name().into_string().unwrap());
                }
            }
            None
        })
        .filter(|entry| entry.is_some())
        .map(|entry| entry.unwrap())
        .collect();
    let mut rng = rand::thread_rng();
    let len = files.len();
    loop {
        let file = files.remove(rng.next_u32() as usize % len);
        let id = u32::from_str(file.split('.').next().unwrap()).unwrap();
        if !previous_ids.contains(&id) {
            previous_ids.push_front(id);
            if previous_ids.len() > 50 {
                previous_ids.pop_back();
            }
            return Ok(file);
        }
    }
}

pub fn get_title_artist(
    mapset_id: u32,
    data: Arc<RwLock<ShareMap>>,
) -> Result<(String, String), Error> {
    let (mut title, artist) = {
        let data = data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Ok(mapset) = mysql.get_beatmapset(mapset_id) {
            Ok((mapset.title, mapset.artist))
        } else {
            let request = BeatmapRequest::new().mapset_id(mapset_id);
            let mut rt = Runtime::new().unwrap();
            let osu = data.get::<Osu>().expect("Could not get Osu");
            match rt.block_on(request.queue_single(&osu)) {
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
    let len = word_a.len().max(word_b.len());
    (len - levenshtein_distance(word_a, word_b)) as f32 / len as f32
}

fn levenshtein_distance(word_a: &str, word_b: &str) -> usize {
    let len_a = word_a.chars().count();
    let len_b = word_b.chars().count();
    if len_a == 0 {
        return len_b;
    } else if len_b == 0 {
        return len_a;
    }
    let mut matrix: Matrix<usize> = Matrix::new(len_b + 1, len_a + 1);
    for x in 0..len_a {
        matrix[(x + 1, 0)] = matrix[(x, 0)] + 1;
    }
    for y in 0..len_b {
        matrix[(0, y + 1)] = matrix[(0, y)] + 1;
    }
    for (x, char_a) in word_a.chars().enumerate() {
        for (y, char_b) in word_b.chars().enumerate() {
            matrix[(x + 1, y + 1)] = (matrix[(x, y + 1)] + 1)
                .min(matrix[(x + 1, y)] + 1)
                .min(matrix[(x, y)] + if char_a == char_b { 0 } else { 1 });
        }
    }
    matrix[(len_a, len_b)]
}
