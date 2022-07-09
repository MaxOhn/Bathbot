use std::f32::consts::SQRT_2;

use eyre::Report;
use image::{GenericImageView, ImageBuffer};
use rand::{prelude::SliceRandom, Rng};
use time::OffsetDateTime;

use crate::{
    core::{ArchivedBytes, Context},
    custom_client::OsuTrackerIdCount,
    BotResult,
};

use super::{kind::GameStateKind, mapset_cover, H, W};

pub type FarmEntries = ArchivedBytes<Vec<OsuTrackerIdCount>>;

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
    ) -> BotResult<Self> {
        let archived = entries.get();

        let (prev_farm, rng_res) = {
            let mut rng = rand::thread_rng();
            let max = archived[0].count as f32;

            let prev_farm = match prev_farm {
                Some(farm) => farm,
                None => rng.gen_range(1..=archived[0].count),
            };

            let rng_res = archived.choose_weighted(&mut rng, |entry| {
                weight(prev_farm, entry.count, max, curr_score)
            });

            (prev_farm, rng_res)
        };

        let (map_id, count) = match rng_res {
            Ok(id_count) => (id_count.map_id, id_count.count),
            Err(err) => {
                let wrap = format!(
                    "failed to choose random entry \
                    (prev={prev_farm} | score={curr_score} | len={})",
                    archived.len()
                );

                let report = Report::new(err).wrap_err(wrap);
                error!("{report:?}");

                (archived[0].map_id, archived[0].count)
            }
        };

        Self::new(ctx, map_id, count).await
    }

    pub async fn image(ctx: &Context, mapset1: u32, mapset2: u32) -> BotResult<String> {
        let cover1 = mapset_cover(mapset1);
        let cover2 = mapset_cover(mapset2);

        // Gather the map covers
        let client = ctx.client();

        let (bg_left, bg_right) = tokio::try_join!(
            client.get_mapset_cover(&cover1),
            client.get_mapset_cover(&cover2),
        )?;

        let bg_left = image::load_from_memory(&bg_left)?;
        let bg_right = image::load_from_memory(&bg_right)?;

        // Combine the images
        let mut blipped = ImageBuffer::new(W, H);

        let iter = blipped
            .enumerate_pixels_mut()
            .zip(bg_left.pixels())
            .zip(bg_right.pixels());

        for (((x, _, pixel), (.., left)), (.., right)) in iter {
            *pixel = if x <= W / 2 { left } else { right };
        }

        let content = format!("{mapset1} ~ {mapset2}",);

        GameStateKind::upload_image(ctx, blipped.as_raw(), content).await
    }

    async fn new(ctx: &Context, map_id: u32, farm: u32) -> BotResult<Self> {
        let map = match ctx.psql().get_beatmap(map_id, true).await {
            Ok(map) => map,
            Err(_) => {
                let map = ctx.osu().beatmap().map_id(map_id).await?;

                if let Err(err) = ctx.psql().insert_beatmap(&map).await {
                    let report = Report::new(err).wrap_err("failed to insert map into DB");
                    warn!("{report:?}");
                }

                map
            }
        };

        let mapset = map.mapset.as_ref().unwrap();

        Ok(Self {
            map_string: format!(
                "{artist} - {title} [{version}]",
                artist = mapset.artist,
                title = mapset.title,
                version = map.version
            ),
            map_url: map.url,
            mapset_id: map.mapset_id,
            stars: map.stars,
            seconds_drain: map.seconds_drain,
            combo: map.max_combo.unwrap_or(0),
            ranked: mapset.ranked_date.unwrap_or_else(OffsetDateTime::now_utc),
            cs: map.cs,
            ar: map.ar,
            od: map.od,
            hp: map.hp,
            farm,
        })
    }
}
