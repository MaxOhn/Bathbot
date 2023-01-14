use std::{array::IntoIter, borrow::Cow, io::Cursor, mem::MaybeUninit};

use bathbot_model::{OsuStatsParams, RespektiveTopCount, ScoreSlim};
use bathbot_util::numbers::WithComma;
use eyre::{Result, WrapErr};
use futures::{stream::FuturesOrdered, StreamExt};
use image::{
    imageops::FilterType, DynamicImage, GenericImage, GenericImageView, ImageOutputFormat,
};
use rosu_pp::{CatchPP, DifficultyAttributes, OsuPP, TaikoPP};
use rosu_v2::prelude::{GameMode, GameMods, Grade, ScoreStatistics};
use time::OffsetDateTime;

use crate::{
    core::{BotConfig, Context},
    embeds::HitResultFormatter,
    manager::{
        redis::{osu::User, RedisData},
        OsuMap,
    },
    util::Emote,
};

use super::ScoreExt;

pub fn grade_emote(grade: Grade) -> &'static str {
    BotConfig::get().grade(grade)
}

pub fn mode_emote(mode: GameMode) -> &'static str {
    let emote = match mode {
        GameMode::Osu => Emote::Std,
        GameMode::Taiko => Emote::Tko,
        GameMode::Catch => Emote::Ctb,
        GameMode::Mania => Emote::Mna,
    };

    emote.text()
}

pub fn grade_completion_mods(
    mods: GameMods,
    grade: Grade,
    score_hits: u32,
    map: &OsuMap,
) -> Cow<'static, str> {
    let mode = map.mode();
    let grade_str = BotConfig::get().grade(grade);

    match (
        mods.is_empty(),
        grade == Grade::F && mode != GameMode::Catch,
    ) {
        (true, true) => format!("{grade_str} ({}%)", completion(score_hits, map)).into(),
        (false, true) => format!("{grade_str} ({}%) +{mods}", completion(score_hits, map)).into(),
        (true, false) => grade_str.into(),
        (false, false) => format!("{grade_str} +{mods}").into(),
    }
}

fn completion(score_hits: u32, map: &OsuMap) -> u32 {
    let total_hits = map.n_objects() as u32;

    100 * score_hits / total_hits
}

pub struct TopCounts {
    pub top1s: Cow<'static, str>,
    pub top1s_rank: Option<String>,
    pub top8s: Cow<'static, str>,
    pub top8s_rank: Option<String>,
    pub top15s: Cow<'static, str>,
    pub top15s_rank: Option<String>,
    pub top25s: Cow<'static, str>,
    pub top25s_rank: Option<String>,
    pub top50s: Cow<'static, str>,
    pub top50s_rank: Option<String>,
    pub top100s: Option<Cow<'static, str>>,
    pub top100s_rank: Option<String>,
    pub last_update: Option<OffsetDateTime>,
}

impl TopCounts {
    pub fn count_len(&self) -> usize {
        let len = self.top50s.len();

        if let Some(ref top100s) = self.top100s {
            len.max(top100s.len())
        } else {
            len
        }
    }

    pub async fn request(ctx: &Context, user: &RedisData<User>, mode: GameMode) -> Result<Self> {
        Self::request_respektive(ctx, user, mode).await
    }

    async fn request_respektive(
        ctx: &Context,
        user: &RedisData<User>,
        mode: GameMode,
    ) -> Result<Self> {
        let counts_fut = ctx
            .client()
            .get_respektive_osustats_counts(user.user_id(), mode);

        match counts_fut.await {
            Ok(Some(counts)) => {
                let top1s = match user {
                    RedisData::Original(user) => user.scores_first_count,
                    RedisData::Archived(user) => user.scores_first_count,
                };

                Ok(Self {
                    top1s: WithComma::new(top1s).to_string().into(),
                    ..Self::from(counts)
                })
            }
            Ok(None) => Ok(Self {
                top1s: "0".into(),
                top1s_rank: None,
                top8s: "0".into(),
                top8s_rank: None,
                top15s: "0".into(),
                top15s_rank: None,
                top25s: "0".into(),
                top25s_rank: None,
                top50s: "0".into(),
                top50s_rank: None,
                top100s: None,
                top100s_rank: None,
                last_update: None,
            }),
            Err(err) => {
                warn!("{:?}", err.wrap_err("failed to get respektive top counts"));

                Self::request_osustats(ctx, user, mode).await
            }
        }
    }

    async fn request_osustats(
        ctx: &Context,
        user: &RedisData<User>,
        mode: GameMode,
    ) -> Result<Self> {
        let mut counts = [
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
        ];

        let mut params = OsuStatsParams::new(user.username()).mode(mode);
        let mut get_amount = true;

        for (rank, count) in [50, 25, 15, 8].into_iter().zip(counts.iter_mut()) {
            if !get_amount {
                count.write("0".into());

                continue;
            }

            params.max_rank = rank;
            let (_, count_) = ctx
                .client()
                .get_global_scores(&params)
                .await
                .wrap_err("failed to get global scores count")?;

            count.write(WithComma::new(count_).to_string().into());

            if count_ == 0 {
                get_amount = false;
            }
        }

        let top1s = match user {
            RedisData::Original(user) => user.scores_first_count,
            RedisData::Archived(user) => user.scores_first_count,
        };

        let top1s = WithComma::new(top1s).to_string().into();

        let [top50s, top25s, top15s, top8s] = counts;

        // SAFETY: All counts were initialized in the loop
        let this = unsafe {
            Self {
                top1s,
                top1s_rank: None,
                top8s: top8s.assume_init(),
                top8s_rank: None,
                top15s: top15s.assume_init(),
                top15s_rank: None,
                top25s: top25s.assume_init(),
                top25s_rank: None,
                top50s: top50s.assume_init(),
                top50s_rank: None,
                top100s: None,
                top100s_rank: None,
                last_update: None,
            }
        };

        Ok(this)
    }
}

impl From<RespektiveTopCount> for TopCounts {
    #[inline]
    fn from(top_count: RespektiveTopCount) -> Self {
        let format_rank = |rank| WithComma::new(rank).to_string();

        Self {
            top1s: WithComma::new(top_count.top1s).to_string().into(),
            top1s_rank: top_count.top1s_rank.map(format_rank),
            top8s: WithComma::new(top_count.top8s).to_string().into(),
            top8s_rank: top_count.top8s_rank.map(format_rank),
            top15s: WithComma::new(top_count.top15s).to_string().into(),
            top15s_rank: top_count.top15s_rank.map(format_rank),
            top25s: WithComma::new(top_count.top25s).to_string().into(),
            top25s_rank: top_count.top25s_rank.map(format_rank),
            top50s: WithComma::new(top_count.top50s).to_string().into(),
            top50s_rank: top_count.top50s_rank.map(format_rank),
            top100s: Some(WithComma::new(top_count.top100s).to_string().into()),
            top100s_rank: top_count.top100s_rank.map(format_rank),
            last_update: Some(top_count.last_update),
        }
    }
}

pub struct TopCount<'a> {
    pub top_n: u8,
    pub count: Cow<'a, str>,
    pub rank: Option<Cow<'a, str>>,
}

pub struct TopCountsIntoIter {
    top_n: IntoIter<u8, 6>,
    counts: IntoIter<Option<Cow<'static, str>>, 6>,
    ranks: IntoIter<Option<String>, 6>,
}

impl Iterator for TopCountsIntoIter {
    type Item = TopCount<'static>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let count = TopCount {
            top_n: self.top_n.next()?,
            count: self.counts.next().flatten()?,
            rank: self.ranks.next()?.map(Cow::Owned),
        };

        Some(count)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.top_n.size_hint()
    }
}

impl IntoIterator for TopCounts {
    type Item = TopCount<'static>;
    type IntoIter = TopCountsIntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let Self {
            top1s,
            top1s_rank,
            top8s,
            top8s_rank,
            top15s,
            top15s_rank,
            top25s,
            top25s_rank,
            top50s,
            top50s_rank,
            top100s,
            top100s_rank,
            last_update: _,
        } = self;

        let top_n = [1, 8, 15, 25, 50, 100];

        let counts = [
            Some(top1s),
            Some(top8s),
            Some(top15s),
            Some(top25s),
            Some(top50s),
            top100s,
        ];

        let ranks = [
            top1s_rank,
            top8s_rank,
            top15s_rank,
            top25s_rank,
            top50s_rank,
            top100s_rank,
        ];

        TopCountsIntoIter {
            top_n: top_n.into_iter(),
            counts: counts.into_iter(),
            ranks: ranks.into_iter(),
        }
    }
}

pub struct TopCountsIter<'a> {
    top_n: IntoIter<u8, 6>,
    counts: IntoIter<Option<&'a str>, 6>,
    ranks: IntoIter<Option<&'a str>, 6>,
}

impl<'a> Iterator for TopCountsIter<'a> {
    type Item = TopCount<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let count = TopCount {
            top_n: self.top_n.next()?,
            count: self.counts.next().flatten().map(Cow::Borrowed)?,
            rank: self.ranks.next()?.map(Cow::Borrowed),
        };

        Some(count)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.top_n.size_hint()
    }
}

impl<'a> IntoIterator for &'a TopCounts {
    type Item = TopCount<'a>;
    type IntoIter = TopCountsIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let TopCounts {
            top1s,
            top1s_rank,
            top8s,
            top8s_rank,
            top15s,
            top15s_rank,
            top25s,
            top25s_rank,
            top50s,
            top50s_rank,
            top100s,
            top100s_rank,
            last_update: _,
        } = self;

        let top_n = [1, 8, 15, 25, 50, 100];

        let counts = [
            Some(top1s.as_ref()),
            Some(top8s.as_ref()),
            Some(top15s.as_ref()),
            Some(top25s.as_ref()),
            Some(top50s.as_ref()),
            top100s.as_deref(),
        ];

        let ranks = [
            top1s_rank.as_deref(),
            top8s_rank.as_deref(),
            top15s_rank.as_deref(),
            top25s_rank.as_deref(),
            top50s_rank.as_deref(),
            top100s_rank.as_deref(),
        ];

        TopCountsIter {
            top_n: top_n.into_iter(),
            counts: counts.into_iter(),
            ranks: ranks.into_iter(),
        }
    }
}

#[derive(Clone)]
pub struct IfFc {
    mode: GameMode,
    pub statistics: ScoreStatistics,
    pub pp: f32,
}

impl IfFc {
    pub async fn new(ctx: &Context, score: &ScoreSlim, map: &OsuMap) -> Option<Self> {
        let mode = score.mode;
        let mut calc = ctx.pp(map).mods(score.mods).mode(score.mode);
        let attrs = calc.difficulty().await;

        if score.is_fc(mode, attrs.max_combo() as u32) {
            return None;
        }

        let mods = score.mods.bits();
        let statistics = &score.statistics;

        let (pp, statistics, mode) = match attrs {
            DifficultyAttributes::Osu(attrs) => {
                let total_objects = map.n_objects();
                let passed_objects = (statistics.count_300
                    + statistics.count_100
                    + statistics.count_50
                    + statistics.count_miss) as usize;

                let mut n300 =
                    statistics.count_300 as usize + total_objects.saturating_sub(passed_objects);

                let count_hits = total_objects - statistics.count_miss as usize;
                let ratio = 1.0 - (n300 as f32 / count_hits as f32);
                let new100s = (ratio * statistics.count_miss as f32).ceil() as u32;

                n300 += statistics.count_miss.saturating_sub(new100s) as usize;
                let n100 = (statistics.count_100 + new100s) as usize;
                let n50 = statistics.count_50 as usize;

                let attrs = OsuPP::new(&map.pp_map)
                    .attributes(attrs.to_owned())
                    .mods(mods)
                    .n300(n300)
                    .n100(n100)
                    .n50(n50)
                    .calculate();

                let statistics = ScoreStatistics {
                    count_300: n300 as u32,
                    count_100: n100 as u32,
                    count_50: n50 as u32,
                    count_geki: statistics.count_geki,
                    count_katu: statistics.count_katu,
                    count_miss: 0,
                };

                (attrs.pp as f32, statistics, GameMode::Osu)
            }
            DifficultyAttributes::Taiko(attrs) => {
                let total_objects = map.n_circles();
                let passed_objects =
                    (statistics.count_300 + statistics.count_100 + statistics.count_miss) as usize;

                let mut n300 =
                    statistics.count_300 as usize + total_objects.saturating_sub(passed_objects);

                let count_hits = total_objects - statistics.count_miss as usize;
                let ratio = 1.0 - (n300 as f32 / count_hits as f32);
                let new100s = (ratio * statistics.count_miss as f32).ceil() as u32;

                n300 += statistics.count_miss.saturating_sub(new100s) as usize;
                let n100 = (statistics.count_100 + new100s) as usize;

                let acc = 100.0 * (2 * n300 + n100) as f32 / (2 * total_objects) as f32;

                let attrs = TaikoPP::new(&map.pp_map)
                    .attributes(attrs.to_owned())
                    .mods(mods)
                    .accuracy(acc as f64)
                    .calculate();

                let statistics = ScoreStatistics {
                    count_300: n300 as u32,
                    count_100: n100 as u32,
                    count_geki: statistics.count_geki,
                    count_katu: statistics.count_katu,
                    count_50: statistics.count_50,
                    count_miss: 0,
                };

                (attrs.pp as f32, statistics, GameMode::Taiko)
            }
            DifficultyAttributes::Catch(attrs) => {
                let total_objects = attrs.max_combo();
                let passed_objects =
                    (statistics.count_300 + statistics.count_100 + statistics.count_miss) as usize;

                let missing = total_objects - passed_objects;
                let missing_fruits = missing.saturating_sub(
                    attrs
                        .n_droplets
                        .saturating_sub(statistics.count_100 as usize),
                );

                let missing_droplets = missing - missing_fruits;

                let n_fruits = statistics.count_300 as usize + missing_fruits;
                let n_droplets = statistics.count_100 as usize + missing_droplets;
                let n_tiny_droplet_misses = statistics.count_katu as usize;
                let n_tiny_droplets = attrs.n_tiny_droplets.saturating_sub(n_tiny_droplet_misses);

                let attrs = CatchPP::new(&map.pp_map)
                    .attributes(attrs.to_owned())
                    .mods(mods)
                    .fruits(n_fruits)
                    .droplets(n_droplets)
                    .tiny_droplets(n_tiny_droplets)
                    .tiny_droplet_misses(n_tiny_droplet_misses)
                    .calculate();

                let statistics = ScoreStatistics {
                    count_300: n_fruits as u32,
                    count_100: n_droplets as u32,
                    count_50: n_tiny_droplets as u32,
                    count_geki: statistics.count_geki,
                    count_katu: statistics.count_katu,
                    count_miss: 0,
                };

                (attrs.pp as f32, statistics, GameMode::Catch)
            }
            DifficultyAttributes::Mania(_) => return None,
        };

        Some(Self {
            mode,
            statistics,
            pp,
        })
    }

    pub fn accuracy(&self) -> f32 {
        self.statistics.accuracy(self.mode)
    }

    pub fn hitresults(&self) -> HitResultFormatter {
        HitResultFormatter::new(self.mode, self.statistics.clone())
    }
}

pub async fn get_combined_thumbnail<'s>(
    ctx: &Context,
    avatar_urls: impl IntoIterator<Item = &'s str>,
    amount: u32,
    width: Option<u32>,
) -> Result<Vec<u8>> {
    let width = width.map_or(128, |w| w.max(128));
    let mut combined = DynamicImage::new_rgba8(width, 128);
    let w = (width / amount).min(128);
    let total_offset = (width - amount * w) / 2;

    // Future stream
    let mut pfp_futs: FuturesOrdered<_> = avatar_urls
        .into_iter()
        .map(|url| ctx.client().get_avatar(url))
        .collect();

    let mut next = pfp_futs.next().await;
    let mut i = 0;

    // Closure that stitches the stripe onto the combined image
    let mut img_combining = |img: DynamicImage, i: u32| {
        let img = img.resize_exact(128, 128, FilterType::Lanczos3);

        let dst_offset = total_offset + i * w;
        let src_offset = (w < 128) as u32 * i * (128 - w) / (amount - 1);

        for i in 0..w {
            for j in 0..128 {
                let pixel = img.get_pixel(src_offset + i, j);
                combined.put_pixel(dst_offset + i, j, pixel);
            }
        }
    };

    // Process the stream elements
    while let Some(pfp_result) = next {
        let pfp = pfp_result?;
        let img = image::load_from_memory(&pfp)?;
        let (res, _) = tokio::join!(pfp_futs.next(), async { img_combining(img, i) });
        next = res;
        i += 1;
    }

    let capacity = width as usize * 128;
    let png_bytes: Vec<u8> = Vec::with_capacity(capacity);
    let mut cursor = Cursor::new(png_bytes);
    combined.write_to(&mut cursor, ImageOutputFormat::Png)?;

    Ok(cursor.into_inner())
}
