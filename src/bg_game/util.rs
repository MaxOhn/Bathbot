use super::GameResult;
use crate::{database::MapsetTagWrapper, util::error::BgGameError, Context};

use rand::RngCore;
use rosu_v2::model::beatmap::BeatmapsetCompact;
use std::collections::VecDeque;

#[allow(clippy::needless_lifetimes)]
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
        let mapset_fut = ctx.psql().get_beatmapset::<BeatmapsetCompact>(mapset_id);

        if let Ok(mapset) = mapset_fut.await {
            (mapset.title.to_lowercase(), mapset.artist)
        } else {
            match ctx.osu().beatmapset(mapset_id).await {
                Ok(mapset) => {
                    if let Err(why) = ctx.psql().insert_beatmapset(&mapset).await {
                        unwind_error!(
                            warn,
                            why,
                            "Error while inserting bg game mapset into DB: {}"
                        );
                    }

                    (mapset.title.to_lowercase(), mapset.artist)
                }
                Err(why) => return Err(BgGameError::Osu(why)),
            }
        }
    };

    if let Some(idx_open) = title.find('(') {
        if let Some(idx_close) = title.rfind(')') {
            title.replace_range(idx_open..=idx_close, "");
        }
    }

    if let Some(idx) = title.find("feat.").or_else(|| title.find("ft.")) {
        title.truncate(idx);
    }

    Ok((title.trim().to_owned(), artist.to_lowercase()))
}
