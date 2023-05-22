use std::{
    f32::consts::SQRT_2,
    fmt::{Display, Formatter, Result as FmtResult},
};

use bathbot_model::OsuTrackerIdCount;
use bathbot_util::{constants::OSU_BASE, datetime::SecToMinSec, EmbedBuilder};
use eyre::{Report, Result, WrapErr};
use image::{GenericImageView, ImageBuffer};
use rand::{prelude::SliceRandom, Rng};
use time::OffsetDateTime;

use super::state::{mapset_cover, HigherLowerState, H, W};
use crate::{core::Context, manager::redis::RedisData};

pub type FarmEntries = RedisData<Vec<OsuTrackerIdCount>>;

pub(super) struct FarmMap {
    pub mapset_id: u32,
    pub farm: u32,
    map_string: Box<str>,
    map_url: Box<str>,
    stars: f32,
    seconds_drain: u32,
    combo: u32,
    ranked: OffsetDateTime,
    cs: f32,
    ar: f32,
    od: f32,
    hp: f32,
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
    pub(super) async fn random(
        ctx: &Context,
        entries: &FarmEntries,
        prev: Option<&Self>,
        curr_score: u32,
    ) -> Result<Self> {
        let prev_farm = prev.map(|game| game.farm);

        let (prev_farm, rng_res) = {
            let mut rng = rand::thread_rng();

            let max = match entries {
                RedisData::Original(entries) => entries[0].count as u32,
                RedisData::Archive(entries) => entries[0].count,
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
                RedisData::Archive(entries) => entries
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
                    RedisData::Archive(entries) => {
                        (entries.len(), entries[0].map_id, entries[0].count)
                    }
                };

                error!(
                    ?err,
                    prev = prev_farm,
                    score = curr_score,
                    len,
                    "Failed to choose random entry"
                );

                (map_id, count)
            }
        };

        Self::new(ctx, map_id, count).await
    }

    pub(super) async fn image(ctx: &Context, mapset_id1: u32, mapset_id2: u32) -> Result<String> {
        let cover1 = mapset_cover(mapset_id1);
        let cover2 = mapset_cover(mapset_id2);

        // Gather the map covers
        let client = ctx.client();

        let (bg_left, bg_right) = tokio::try_join!(
            client.get_mapset_cover(&cover1),
            client.get_mapset_cover(&cover2),
        )
        .wrap_err("Failed to get mapset cover")?;

        let bg_left =
            image::load_from_memory(&bg_left).wrap_err("Failed to load left bg from memory")?;

        let bg_right =
            image::load_from_memory(&bg_right).wrap_err("Failed to load right bg from memory")?;

        // Combine the images
        let mut blipped = ImageBuffer::new(W, H);

        let iter = blipped
            .enumerate_pixels_mut()
            .zip(bg_left.pixels())
            .zip(bg_right.pixels());

        for (((x, _, pixel), (.., left)), (.., right)) in iter {
            *pixel = if x <= W / 2 { left } else { right };
        }

        let content = format!("{mapset_id1} ~ {mapset_id2}");

        HigherLowerState::upload_image(ctx, blipped.as_raw(), content).await
    }

    pub(super) fn log(game1: &Self, game2: &Self) {
        debug!("farm: {} vs {}", game1.farm, game2.farm);
    }

    pub(super) fn to_embed(previous: &Self, next: &Self, revealed: bool) -> EmbedBuilder {
        let description = format!(
            "**__Previous:__ [{prev_map}]({prev_url})**\n\
            `{prev_stars:.2}★` • `{prev_len}` • `{prev_combo}x` • Ranked <t:{prev_timestamp}:D>\n\
            `CS {prev_cs}` `AR {prev_ar}` `OD {prev_od}` `HP {prev_hp}` • {prev_farm}\n\
            **__Next:__ [{next_map}]({next_url})**\n\
            `{next_stars:.2}★` • `{next_len}` • `{next_combo}x` • Ranked <t:{next_timestamp}:D>\n\
            `CS {next_cs}` `AR {next_ar}` `OD {next_od}` `HP {next_hp}` • {next_farm}",
            prev_map = previous.map_string,
            prev_url = previous.map_url,
            prev_stars = previous.stars,
            prev_len = SecToMinSec::new(previous.seconds_drain),
            prev_combo = previous.combo,
            prev_timestamp = previous.ranked.unix_timestamp(),
            prev_cs = previous.cs,
            prev_ar = previous.ar,
            prev_od = previous.od,
            prev_hp = previous.hp,
            prev_farm = FarmFormatter::new(previous.farm, true),
            next_map = next.map_string,
            next_url = next.map_url,
            next_stars = next.stars,
            next_len = SecToMinSec::new(next.seconds_drain),
            next_combo = next.combo,
            next_timestamp = next.ranked.unix_timestamp(),
            next_cs = next.cs,
            next_ar = next.ar,
            next_od = next.od,
            next_hp = next.hp,
            next_farm = FarmFormatter::new(next.farm, revealed),
        );

        EmbedBuilder::new().description(description)
    }

    async fn new(ctx: &Context, map_id: u32, farm: u32) -> Result<Self> {
        let map = match ctx.osu_map().map(map_id, None).await {
            Ok(map) => map,
            Err(err) => return Err(Report::new(err).wrap_err("Failed to get beatmap")),
        };

        let mut calc = ctx.pp(&map);
        let attrs = calc.difficulty().await;

        Ok(Self {
            map_string: format!(
                "{artist} - {title} [{version}]",
                artist = map.artist(),
                title = map.title(),
                version = map.version(),
            )
            .into_boxed_str(),
            map_url: format!("{OSU_BASE}b/{}", map.map_id()).into_boxed_str(),
            mapset_id: map.mapset_id(),
            stars: attrs.stars() as f32,
            seconds_drain: map.seconds_drain(),
            combo: attrs.max_combo() as u32,
            ranked: map.ranked_date().unwrap_or_else(OffsetDateTime::now_utc),
            cs: map.cs(),
            ar: map.ar(),
            od: map.od(),
            hp: map.hp(),
            farm,
        })
    }
}

struct FarmFormatter {
    farm: u32,
    revealed: bool,
}

impl FarmFormatter {
    fn new(farm: u32, revealed: bool) -> Self {
        Self { farm, revealed }
    }
}

impl Display for FarmFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.revealed {
            write!(
                f,
                "In **{}** top score{}",
                self.farm,
                if self.farm != 1 { "s" } else { "" }
            )
        } else {
            f.write_str("In **???** top scores")
        }
    }
}
