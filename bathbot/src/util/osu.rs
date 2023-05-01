use std::{
    array::IntoIter,
    borrow::Cow,
    cmp::Ordering,
    convert::identity,
    fmt::{Display, Formatter, Result as FmtResult},
    io::Cursor,
    mem::MaybeUninit,
};

use bathbot_model::{rosu_v2::user::User, OsuStatsParams, ScoreSlim};
use bathbot_util::{
    datetime::SecToMinSec,
    numbers::{round, WithComma},
    ScoreExt,
};
use eyre::{Result, WrapErr};
use futures::{stream::FuturesOrdered, StreamExt};
use image::{
    imageops::FilterType, DynamicImage, GenericImage, GenericImageView, ImageOutputFormat,
};
use rosu_pp::{
    beatmap::BeatmapAttributesBuilder, CatchPP, DifficultyAttributes, GameMode as Mode, OsuPP,
    TaikoPP,
};
use rosu_v2::prelude::{GameMode, GameMods, Grade, RankStatus, Score, ScoreStatistics};
use time::OffsetDateTime;

use crate::{
    core::{BotConfig, Context},
    embeds::{HitResultFormatter, MessageOrigin},
    manager::{redis::RedisData, OsuMap},
    util::Emote,
};

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
    mods: &GameMods,
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
    pub top100s: Cow<'static, str>,
    pub top100s_rank: Option<String>,
    pub last_update: Option<OffsetDateTime>,
}

impl TopCounts {
    pub fn count_len(&self) -> usize {
        self.top100s.len()
    }

    pub async fn request(ctx: &Context, user: &RedisData<User>, mode: GameMode) -> Result<Self> {
        Self::request_osustats(ctx, user, mode).await
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
            MaybeUninit::uninit(),
        ];

        let mut params = OsuStatsParams::new(user.username()).mode(mode);
        let mut get_amount = true;

        for (rank, count) in [100, 50, 25, 15, 8].into_iter().zip(counts.iter_mut()) {
            if !get_amount {
                count.write("0".into());

                continue;
            }

            params.max_rank = rank;
            let (_, count_) = ctx
                .client()
                .get_global_scores(&params)
                .await
                .wrap_err("Failed to get global scores count")?;

            count.write(WithComma::new(count_).to_string().into());

            if count_ == 0 {
                get_amount = false;
            }
        }

        let top1s = match user {
            RedisData::Original(user) => user.scores_first_count,
            RedisData::Archive(user) => user.scores_first_count,
        };

        let top1s = WithComma::new(top1s).to_string().into();

        let [top100s, top50s, top25s, top15s, top8s] = counts;

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
                top100s: top100s.assume_init(),
                top100s_rank: None,
                last_update: None,
            }
        };

        Ok(this)
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
    type IntoIter = TopCountsIntoIter;
    type Item = TopCount<'static>;

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
            Some(top100s),
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
    type IntoIter = TopCountsIter<'a>;
    type Item = TopCount<'a>;

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
            Some(top100s.as_ref()),
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
        let mut calc = ctx.pp(map).mods(score.mods.bits()).mode(score.mode);
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

        let src_offset = if amount == 1 {
            0
        } else {
            (w < 128) as u32 * i * (128 - w) / (amount - 1)
        };

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

pub struct MapInfo<'map> {
    map: &'map OsuMap,
    stars: f32,
    mods: Option<u32>,
    clock_rate: Option<f32>,
}

impl<'map> MapInfo<'map> {
    pub fn new(map: &'map OsuMap, stars: f32) -> Self {
        Self {
            map,
            stars,
            mods: None,
            clock_rate: None,
        }
    }

    pub fn mods(&mut self, mods: u32) -> &mut Self {
        self.mods = Some(mods);

        self
    }

    pub fn clock_rate(&mut self, clock_rate: f32) -> &mut Self {
        self.clock_rate = Some(clock_rate);

        self
    }
}

impl Display for MapInfo<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let mode = match self.map.mode() {
            GameMode::Osu => Mode::Osu,
            GameMode::Taiko => Mode::Taiko,
            GameMode::Catch => Mode::Catch,
            GameMode::Mania => Mode::Mania,
        };

        let mods = self.mods.unwrap_or(0);

        let mut builder = BeatmapAttributesBuilder::new(&self.map.pp_map);

        if let Some(clock_rate) = self.clock_rate {
            builder.clock_rate(clock_rate as f64);
        }

        let attrs = builder.mode(mode).mods(mods).build();

        let clock_rate = attrs.clock_rate;
        let mut sec_drain = self.map.seconds_drain();
        let mut bpm = self.map.bpm();

        if (clock_rate - 1.0).abs() > f64::EPSILON {
            let clock_rate = clock_rate as f32;

            bpm *= clock_rate;
            sec_drain = (sec_drain as f32 / clock_rate) as u32;
        }

        write!(
            f,
            "Length: `{}` BPM: `{}` Objects: `{}`\n\
            CS: `{}` AR: `{}` OD: `{}` HP: `{}` Stars: `{}`",
            SecToMinSec::new(sec_drain),
            round(bpm),
            self.map.n_objects(),
            round(attrs.cs as f32),
            round(attrs.ar as f32),
            round(attrs.od as f32),
            round(attrs.hp as f32),
            round(self.stars),
        )
    }
}

/// Note that all contained indices start at 0.
pub enum PersonalBestIndex {
    /// Found the score in the top100
    FoundScore { idx: usize },
    /// There was a score on the same map with more pp in the top100
    FoundBetter { idx: usize },
    /// Found another score on the same map and the
    /// same mods that has more score but less pp
    ScoreV1d { would_be_idx: usize, old_idx: usize },
    /// Score is ranked and has enough pp to be in but wasn't found
    Presumably { idx: usize },
    /// Score is not ranked but has enough pp to be in the top100
    IfRanked { idx: usize },
    /// Score does not have enough pp to be in the top100
    NotTop100,
}

impl PersonalBestIndex {
    pub fn new(score: &ScoreSlim, map_id: u32, status: RankStatus, top100: &[Score]) -> Self {
        // Note that the index is determined through float
        // comparisons which could result in issues
        let idx = top100
            .binary_search_by(|probe| {
                probe
                    .pp
                    .and_then(|pp| score.pp.partial_cmp(&pp))
                    .unwrap_or(Ordering::Less)
            })
            .unwrap_or_else(identity);

        if idx == 100 {
            return Self::NotTop100;
        } else if !matches!(status, RankStatus::Ranked | RankStatus::Approved) {
            return Self::IfRanked { idx };
        } else if top100.get(idx).filter(|&top| score.is_eq(top)).is_some() {
            // If multiple scores have the exact same pp as the given
            // score then `idx` might not belong to the given score.
            // Chances are pretty slim though so this should be fine.
            return Self::FoundScore { idx };
        }

        let (better, worse) = top100.split_at(idx);

        // A case that's not covered is when there is a score
        // with more pp on the same map with the same mods that has
        // less score than the current score. Sounds really fringe though.
        if let Some(idx) = better.iter().position(|top| top.map_id == map_id) {
            Self::FoundBetter { idx }
        } else if let Some(i) = worse.iter().position(|top| {
            top.map_id == map_id && top.mods == score.mods && top.score > score.score
        }) {
            Self::ScoreV1d {
                would_be_idx: idx,
                old_idx: idx + i,
            }
        } else {
            Self::Presumably { idx }
        }
    }

    pub fn into_embed_description(self, origin: &MessageOrigin) -> Option<String> {
        match self {
            PersonalBestIndex::FoundScore { idx } => Some(format!("Personal Best #{}", idx + 1)),
            PersonalBestIndex::FoundBetter { .. } => None,
            PersonalBestIndex::ScoreV1d {
                would_be_idx,
                old_idx,
            } => Some(format!(
                "Personal Best #{idx} ([v1'd]({origin} \
                \"there is a play on the same map with the same mods that has more score\"\
                ) by #{old})",
                idx = would_be_idx + 1,
                old = old_idx + 1
            )),
            PersonalBestIndex::Presumably { idx } => Some(format!(
                "Personal Best #{} [(?)]({origin} \
                \"the top100 did not include this score likely because the api \
                wasn't done processing but presumably the score is in there\")",
                idx + 1
            )),
            PersonalBestIndex::IfRanked { idx } => {
                Some(format!("Personal Best #{} (if ranked)", idx + 1))
            }
            PersonalBestIndex::NotTop100 => None,
        }
    }
}
