use std::f32::consts::SQRT_2;

use chrono::{DateTime, Utc};
use eyre::Report;
use hashbrown::HashMap;
use image::{GenericImageView, ImageBuffer};
use rand::{prelude::SliceRandom, Rng};

use crate::{core::Context, BotResult};

use super::{kind::GameStateKind, mapset_cover, H, W};

pub(super) struct FarmEntries {
    max: f32,
    entries: Vec<FarmEntry>,
}

#[derive(Copy, Clone)]
struct FarmEntry {
    map_id: u32,
    count: u32,
}

impl FarmEntries {
    pub(super) async fn new(ctx: &Context) -> BotResult<Self> {
        let mut max = 0;

        let entries = ctx
            .redis()
            .osutracker_groups()
            .await?
            .get()
            .iter()
            .flat_map(|group| group.list.iter())
            .fold(
                HashMap::<u32, u32>::with_capacity(2048),
                |mut map, entry| {
                    let entry_ = map.entry(entry.map_id).or_default();
                    *entry_ += entry.count;
                    max = max.max(*entry_);

                    map
                },
            )
            .into_iter()
            .map(|(map_id, count)| FarmEntry { map_id, count })
            .collect();

        let max = max as f32;

        Ok(Self { max, entries })
    }
}

pub(super) struct FarmMap {
    pub map_string: String,
    pub map_url: String,
    pub mapset_id: u32,
    pub stars: f32,
    pub seconds_drain: u32,
    pub combo: u32,
    pub ranked: DateTime<Utc>,
    pub cs: f32,
    pub ar: f32,
    pub od: f32,
    pub hp: f32,
    pub farm: u32,
}

impl FarmMap {
    pub async fn random(
        ctx: &Context,
        entries: &FarmEntries,
        prev_farm: Option<u32>,
        curr_score: u32,
    ) -> BotResult<Self> {
        let (prev_farm, rng_res) = {
            let mut rng = rand::thread_rng();

            let prev_farm = match prev_farm {
                Some(farm) => farm,
                None => rng.gen_range(1..=entries.max as u32),
            };

            let rng_res = entries.entries.choose_weighted(&mut rng, |entry| {
                if entry.count == prev_farm {
                    return 0.0;
                }

                // https://www.desmos.com/calculator/0t2lt97bnh
                const THRESHOLD: f32 = 25.0;
                const CAP: f32 = 1500.0;
                const EXP: f32 = 0.7;
                const FACTOR: f32 = 25.0;

                let factor = entries.max * FACTOR;
                let handicap = (THRESHOLD - curr_score as f32).max(1.0);
                let percent = (THRESHOLD - handicap) / THRESHOLD;

                let region = entry.count.abs_diff(prev_farm);
                let main = (factor * handicap / region as f32).powf(EXP).min(CAP);
                let invert_base = ((percent - 1.0) * SQRT_2) + 1.0;
                let invert = invert_base * invert_base + 1.0;
                let offset = CAP * (1.0 - percent);

                main * invert + offset
            });

            (prev_farm, rng_res)
        };

        let entry = match rng_res {
            Ok(tuple) => *tuple,
            Err(err) => {
                let wrap = format!(
                    "failed to choose random entry \
                    (prev={prev_farm} | score={curr_score} | len={})",
                    entries.entries.len()
                );

                let report = Report::new(err).wrap_err(wrap);
                error!("{report:?}");

                entries.entries[0]
            }
        };

        Self::new(ctx, entry).await
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

    async fn new(ctx: &Context, entry: FarmEntry) -> BotResult<Self> {
        let map = match ctx.psql().get_beatmap(entry.map_id, true).await {
            Ok(map) => map,
            Err(_) => {
                let map = ctx.osu().beatmap().map_id(entry.map_id).await?;

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
            ranked: mapset.ranked_date.unwrap_or_else(Utc::now),
            cs: map.cs,
            ar: map.ar,
            od: map.od,
            hp: map.hp,
            farm: entry.count,
        })
    }
}
