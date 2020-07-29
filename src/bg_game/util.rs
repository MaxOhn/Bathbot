use super::GameResult;
use crate::{
    database::MapsetTagWrapper,
    util::{error::BgGameError, levenshtein_distance},
    Context,
};

use rand::RngCore;
use rosu::backend::BeatmapRequest;
use std::collections::VecDeque;

pub async fn get_random_mapset<'m>(
    mapsets: &'m [MapsetTagWrapper],
    previous_ids: &mut VecDeque<u32>,
) -> &'m MapsetTagWrapper {
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
            return mapset;
        }
    }
}

pub async fn get_title_artist(ctx: &Context, mapset_id: u32) -> GameResult<(String, String)> {
    let (mut title, artist) = {
        if let Ok(mapset) = ctx.psql().get_beatmapset(mapset_id).await {
            (mapset.title, mapset.artist)
        } else {
            let request = BeatmapRequest::new().mapset_id(mapset_id);
            match request.queue_single(ctx.osu()).await {
                Ok(Some(map)) => (map.title, map.artist),
                Ok(None) => return Err(BgGameError::NoMapResult(mapset_id)),
                Err(why) => return Err(BgGameError::Osu(why)),
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
