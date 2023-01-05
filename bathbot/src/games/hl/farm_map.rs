use std::f32::consts::SQRT_2;

use bathbot_model::OsuTrackerIdCount;
use bathbot_util::constants::OSU_BASE;
use eyre::{Report, Result, WrapErr};
use image::{GenericImageView, ImageBuffer};
use rand::{prelude::SliceRandom, Rng};
use time::OffsetDateTime;

use crate::{core::Context, manager::redis::RedisData};

use super::{kind::GameStateKind, mapset_cover, H, W};

pub type FarmEntries = RedisData<Vec<OsuTrackerIdCount>>;

pub(super) struct FarmMap {
    pub map_string: String,
    pub map_url: String,
    pub mapset_id: u32,
    pub stars: f32,
    pub seconds_drain: u32,
    pub combo: u32,
    pub ranked: OffsetDateTime,
    pub cs: f32,
    pub ar: f32,
    pub od: f32,
    pub hp: f32,
    pub farm: u32,
}

const THRESHOLD: f32 = 25.0;
const CAP: f32 = 2000.0;
const EXP: f32 = 0.7;
const FACTOR: f32 = 100.0;

// https://www.desmos.com/calculator/u4jt9t4jnj
fn weight(prev: u32, curr: u32, max: f32, score: u32) -> f32 {
    if curr <= 2 || curr == prev {
        return 0.0;
    }

    let factor = max * FACTOR;
    let handicap = (THRESHOLD - score as f32).max(1.0);
    let percent = (THRESHOLD - handicap) / THRESHOLD;

    let region = curr.abs_diff(prev);
    let main = (factor * handicap / region as f32).powf(EXP).min(CAP);
    let invert_base = ((percent - 1.0) * SQRT_2) + 1.0;
    let invert = invert_base * invert_base + 1.0;
    let offset = CAP * (1.0 - percent);
    let log_shift = (curr as f32).ln().powi(4) / CAP;

    (main * invert + offset) * log_shift
}

impl FarmMap {
    pub async fn random(
        ctx: &Context,
        entries: &FarmEntries,
        prev_farm: Option<u32>,
        curr_score: u32,
    ) -> Result<Self> {
        let (prev_farm, rng_res) = {
            let mut rng = rand::thread_rng();

            let max = match entries {
                RedisData::Original(entries) => entries[0].count as u32,
                RedisData::Archived(entries) => entries[0].count,
            };

            let prev_farm = match prev_farm {
                Some(farm) => farm,
                None => rng.gen_range(1..=max),
            };

            let max = max as f32;

            let rng_res = match entries {
                RedisData::Original(entries) => entries
                    .choose_weighted(&mut rng, |entry| {
                        weight(prev_farm, entry.count as u32, max, curr_score)
                    })
                    .map(|id_count| (id_count.map_id, id_count.count as u32)),
                RedisData::Archived(entries) => entries
                    .choose_weighted(&mut rng, |entry| {
                        weight(prev_farm, entry.count, max, curr_score)
                    })
                    .map(|id_count| (id_count.map_id, id_count.count)),
            };

            (prev_farm, rng_res)
        };

        let (map_id, count) = match rng_res {
            Ok(tuple) => tuple,
            Err(err) => {
                let (len, map_id, count) = match entries {
                    RedisData::Original(entries) => {
                        (entries.len(), entries[0].map_id, entries[0].count as u32)
                    }
                    RedisData::Archived(entries) => {
                        (entries.len(), entries[0].map_id, entries[0].count)
                    }
                };

                let wrap = format!(
                    "failed to choose random entry \
                    (prev={prev_farm} | score={curr_score} | len={len})",
                );

                let report = Report::new(err).wrap_err(wrap);
                error!("{report:?}");

                (map_id, count)
            }
        };

        Self::new(ctx, map_id, count).await
    }

    pub async fn image(ctx: &Context, mapset1: u32, mapset2: u32) -> Result<String> {
        let cover1 = mapset_cover(mapset1);
        let cover2 = mapset_cover(mapset2);

        // Gather the map covers
        let client = ctx.client();

        let (bg_left, bg_right) = tokio::try_join!(
            client.get_mapset_cover(&cover1),
            client.get_mapset_cover(&cover2),
        )
        .wrap_err("failed to get mapset cover")?;

        let bg_left =
            image::load_from_memory(&bg_left).wrap_err("failed to load left bg from memory")?;

        let bg_right =
            image::load_from_memory(&bg_right).wrap_err("failed to load right bg from memory")?;

        // Combine the images
        let mut blipped = ImageBuffer::new(W, H);

        let iter = blipped
            .enumerate_pixels_mut()
            .zip(bg_left.pixels())
            .zip(bg_right.pixels());

        for (((x, _, pixel), (.., left)), (.., right)) in iter {
            *pixel = if x <= W / 2 { left } else { right };
        }

        let content = format!("{mapset1} ~ {mapset2}");

        GameStateKind::upload_image(ctx, blipped.as_raw(), content).await
    }

    async fn new(ctx: &Context, map_id: u32, farm: u32) -> Result<Self> {
        let map = match ctx.osu_map().map(map_id, None).await {
            Ok(map) => map,
            Err(err) => return Err(Report::new(err).wrap_err("failed to get beatmap")),
        };

        let stars = ctx.pp(&map).difficulty().await.stars() as f32;

        Ok(Self {
            map_string: format!(
                "{artist} - {title} [{version}]",
                artist = map.artist(),
                title = map.title(),
                version = map.version(),
            ),
            map_url: format!("{OSU_BASE}b/{}", map.map_id()),
            mapset_id: map.mapset_id(),
            stars,
            seconds_drain: map.seconds_drain(),
            combo: map.max_combo().unwrap_or(0),
            ranked: map.ranked_date().unwrap_or_else(OffsetDateTime::now_utc),
            cs: map.cs(),
            ar: map.ar(),
            od: map.od(),
            hp: map.hp(),
            farm,
        })
    }
}
