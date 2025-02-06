use std::{
    array::IntoIter,
    borrow::Cow,
    cmp::Ordering,
    convert::identity,
    fmt::{Display, Formatter, Result as FmtResult},
    io::Cursor,
    mem::MaybeUninit,
};

use bathbot_model::{OsuStatsParams, ScoreSlim};
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    constants::OSU_BASE,
    datetime::SecToMinSec,
    matcher,
    numbers::{round, WithComma},
    osu::MapIdType,
    MessageOrigin, ModsFormatter, ScoreExt,
};
use eyre::{Result, WrapErr};
use futures::{stream::FuturesOrdered, StreamExt};
use image::{
    imageops::FilterType, DynamicImage, GenericImage, GenericImageView, ImageOutputFormat,
};
use rosu_pp::{
    any::DifficultyAttributes, catch::CatchPerformance, osu::OsuPerformance,
    taiko::TaikoPerformance,
};
use rosu_v2::{
    model::mods::GameMods,
    prelude::{GameModIntermode, GameMode, Grade, RankStatus, Score, ScoreStatistics},
};
use time::OffsetDateTime;
use twilight_model::channel::{message::MessageType, Message};

use crate::{
    core::{BotConfig, Context},
    manager::{redis::osu::CachedUser, OsuMap},
};

pub fn grade_emote(grade: Grade) -> &'static str {
    BotConfig::get().grade(grade)
}

pub struct GradeCompletionFormatter<'a> {
    mods: &'a GameMods,
    grade: Grade,
    score_hits: u32,
    mode: GameMode,
    n_objects: u32,
    score_id: Option<u64>,
}

impl<'a> GradeCompletionFormatter<'a> {
    pub fn new<S: ScoreExt>(score: &'a S, mode: GameMode, n_objects: u32) -> Self {
        Self {
            mods: score.mods(),
            grade: score.grade(),
            score_hits: score.total_hits(mode as u8),
            mode,
            n_objects,
            score_id: score.score_id().filter(|_| !score.is_legacy()),
        }
    }

    /// Careful about the grade!
    ///
    /// The osu!api no longer uses `Grade::F` but this method expects `Grade::F`
    /// for fails.
    pub fn new_without_score(
        mods: &'a GameMods,
        grade: Grade,
        score_hits: u32,
        mode: GameMode,
        n_objects: u32,
    ) -> Self {
        Self {
            mods,
            grade,
            score_hits,
            mode,
            n_objects,
            score_id: None,
        }
    }
}

impl Display for GradeCompletionFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let completion = || {
            if self.n_objects != 0 {
                100 * self.score_hits / self.n_objects
            } else {
                100
            }
        };

        let grade_fmt = GradeFormatter {
            grade: self.grade,
            score_id: self.score_id,
        };

        let mods_fmt = ModsFormatter::new(self.mods);

        // The completion is very hard to calculate for `Catch` because
        // `n_objects` is not correct due to juicestreams so we won't
        // show it for that mode.
        let is_fail = self.grade == Grade::F && self.mode != GameMode::Catch;

        match (self.mods.is_empty(), is_fail) {
            (true, true) => write!(f, "{grade_fmt}@{}%", completion()),
            (false, true) => write!(f, "{grade_fmt}@{}% +{mods_fmt}", completion()),
            (true, false) => Display::fmt(&grade_fmt, f),
            (false, false) => write!(f, "{grade_fmt} +{mods_fmt}"),
        }
    }
}

/// Format a grade's emote and optionally hyperlink to the score if the id is
/// available.
pub struct GradeFormatter {
    grade: Grade,
    score_id: Option<u64>,
}

impl GradeFormatter {
    pub fn new(grade: Grade, score_id: Option<u64>, is_legacy: bool) -> Self {
        Self {
            grade,
            score_id: score_id.filter(|_| !is_legacy),
        }
    }
}

impl Display for GradeFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let grade = grade_emote(self.grade);

        match self.score_id {
            Some(score_id) => write!(f, "[{grade}]({OSU_BASE}scores/{score_id})"),
            None => f.write_str(grade),
        }
    }
}

#[derive(Copy, Clone)]
pub struct ScoreFormatter {
    score: u64,
}

impl ScoreFormatter {
    pub fn new(score: &ScoreSlim, score_data: ScoreData) -> Self {
        let score = match score_data {
            ScoreData::Stable | ScoreData::Lazer => score.score as u64,
            ScoreData::LazerWithClassicScoring if score.classic_score == 0 => score.score as u64,
            ScoreData::LazerWithClassicScoring => score.classic_score,
        };

        Self { score }
    }
}

impl Display for ScoreFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&WithComma::new(self.score), f)
    }
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

    pub async fn request(user: &CachedUser, mode: GameMode) -> Result<Self> {
        Self::request_osustats(user, mode).await
    }

    async fn request_osustats(user: &CachedUser, mode: GameMode) -> Result<Self> {
        let mut counts = [
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
        ];

        let mut params = OsuStatsParams::new(user.username.as_str());
        params.mode(mode);
        let mut params_clone = params.clone();
        let mut get_amount = true;

        let mut iter = [100, 50, 25, 15, 8].into_iter().zip(counts.iter_mut());

        // Try to request 2 ranks concurrently
        while let Some((next_rank, next_count)) = iter.next() {
            if !get_amount {
                next_count.write("0".into());

                continue;
            }

            params.max_rank(next_rank);
            let next_fut = Context::client().get_global_scores(&params);

            let count = match iter.next() {
                Some((next_next_rank, next_next_count)) => {
                    params_clone.max_rank(next_next_rank);

                    let next_next_fut = Context::client().get_global_scores(&params_clone);

                    let (next_raw, next_next_raw) = tokio::try_join!(next_fut, next_next_fut)
                        .wrap_err("Failed to get global scores count")?;

                    let next_count_ = next_raw.count()?;
                    let next_next_count_ = next_next_raw.count()?;

                    next_count.write(WithComma::new(next_count_).to_string().into());
                    next_next_count.write(WithComma::new(next_next_count_).to_string().into());

                    next_next_count_
                }
                None => {
                    let next_raw = next_fut
                        .await
                        .wrap_err("Failed to get global scores count")?;

                    let next_count_ = next_raw.count()?;
                    next_count.write(WithComma::new(next_count_).to_string().into());

                    next_count_
                }
            };

            if count == 0 {
                get_amount = false;
            }
        }

        let top1s = WithComma::new(user.scores_first_count.to_native())
            .to_string()
            .into();

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
    pub statistics: ScoreStatistics,
    pub max_statistics: Option<ScoreStatistics>,
    pub pp: f32,
}

impl IfFc {
    pub async fn new(score: &ScoreSlim, map: &OsuMap) -> Option<Self> {
        let mut calc = Context::pp(map)
            .mods(score.mods.clone())
            .mode(score.mode)
            .lazer(score.set_on_lazer);

        let attrs = calc.difficulty().await;

        if score.is_fc(score.mode, attrs.max_combo()) {
            return None;
        }

        let stats = &score.statistics;

        let (pp, statistics) = match attrs {
            DifficultyAttributes::Osu(attrs) => {
                let total_objects = map.n_objects();
                let passed_objects = stats.great + stats.ok + stats.meh + stats.miss;

                let mut n300 = stats.great + total_objects.saturating_sub(passed_objects);

                let count_hits = total_objects - stats.miss;
                let ratio = 1.0 - (n300 as f32 / count_hits as f32);
                let new100s = (ratio * stats.miss as f32).ceil() as u32;

                n300 += stats.miss.saturating_sub(new100s);
                let n100 = stats.ok + new100s;
                let n50 = stats.meh;

                let classic = score.mods.contains_intermode(GameModIntermode::Classic);

                let attrs = OsuPerformance::from(attrs.to_owned())
                    .lazer(score.set_on_lazer)
                    .mods(score.mods.clone())
                    .n300(n300)
                    .n100(n100)
                    .n50(n50)
                    .slider_end_hits(score.statistics.slider_tail_hit)
                    .small_tick_hits(score.statistics.small_tick_hit)
                    // no large tick misses allowed for fc so we can omit that
                    .calculate()
                    .unwrap();

                let mut statistics = stats.clone();
                statistics.great = n300;
                statistics.ok = n100;
                statistics.meh = n50;
                statistics.miss = 0;
                statistics.large_tick_hit = attrs.difficulty.n_large_ticks;
                statistics.large_tick_miss = 0;

                if classic {
                    statistics.slider_tail_hit = attrs.difficulty.n_sliders;
                } else {
                    statistics.small_tick_hit = attrs.difficulty.n_sliders;
                }

                (attrs.pp as f32, statistics)
            }
            DifficultyAttributes::Taiko(attrs) => {
                let total_objects = map.n_circles();
                let passed_objects = (stats.great + stats.ok + stats.miss) as usize;

                let mut n300 = stats.great as usize + total_objects.saturating_sub(passed_objects);

                let count_hits = total_objects - stats.miss as usize;
                let ratio = 1.0 - (n300 as f32 / count_hits as f32);
                let new100s = (ratio * stats.miss as f32).ceil() as u32;

                n300 += stats.miss.saturating_sub(new100s) as usize;
                let n100 = (stats.ok + new100s) as usize;

                let acc = 100.0 * (2 * n300 + n100) as f32 / (2 * total_objects) as f32;

                let attrs = TaikoPerformance::from(attrs.to_owned())
                    .mods(score.mods.clone())
                    .accuracy(acc as f64)
                    .calculate()
                    .unwrap();

                let mut statistics = stats.clone();
                statistics.great = n300 as u32;
                statistics.ok = n100 as u32;
                statistics.miss = 0;

                (attrs.pp as f32, statistics)
            }
            DifficultyAttributes::Catch(attrs) => {
                let total_objects = attrs.max_combo();
                let passed_objects = stats.great + stats.ok + stats.miss;

                let missing = total_objects - passed_objects;
                let missing_fruits =
                    missing.saturating_sub(attrs.n_droplets.saturating_sub(stats.ok));

                let missing_droplets = missing - missing_fruits;

                let n_fruits = stats.great + missing_fruits;
                let n_droplets = stats.ok + missing_droplets;
                let n_tiny_droplet_misses = stats.small_tick_miss.max(stats.good);
                let n_tiny_droplets = attrs.n_tiny_droplets.saturating_sub(n_tiny_droplet_misses);

                let attrs = CatchPerformance::from(attrs.to_owned())
                    .mods(score.mods.clone())
                    .fruits(n_fruits)
                    .droplets(n_droplets)
                    .tiny_droplets(n_tiny_droplets)
                    .tiny_droplet_misses(n_tiny_droplet_misses)
                    .calculate()
                    .unwrap();

                let mut statistics = stats.clone();
                statistics.great = n_fruits;
                statistics.ok = n_droplets;
                statistics.meh = n_tiny_droplets;
                statistics.miss = 0;

                (attrs.pp as f32, statistics)
            }
            DifficultyAttributes::Mania(_) => return None,
        };

        let max_statistics = score.set_on_lazer.then(|| {
            let total_hits = score.total_hits();

            match attrs {
                DifficultyAttributes::Osu(attrs) => ScoreStatistics {
                    great: total_hits,
                    large_tick_hit: attrs.n_large_ticks,
                    small_tick_hit: attrs.n_sliders,
                    slider_tail_hit: attrs.n_sliders,
                    ..Default::default()
                },
                DifficultyAttributes::Taiko(_) => ScoreStatistics {
                    great: map.n_circles() as u32,
                    ..Default::default()
                },
                DifficultyAttributes::Catch(attrs) => ScoreStatistics {
                    great: attrs.n_fruits,
                    ok: attrs.n_droplets,
                    meh: attrs.n_tiny_droplets,
                    ..Default::default()
                },
                DifficultyAttributes::Mania(_) => ScoreStatistics {
                    perfect: total_hits,
                    ..Default::default()
                },
            }
        });

        Some(Self {
            statistics,
            max_statistics,
            pp,
        })
    }
}

pub async fn get_combined_thumbnail<'s>(
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
        .map(|url| Context::client().get_avatar(url))
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

pub struct MapInfo<'a> {
    map: &'a OsuMap,
    stars: f32,
    mods: Option<&'a GameMods>,
    clock_rate: Option<f64>,
}

impl<'a> MapInfo<'a> {
    pub fn new(map: &'a OsuMap, stars: f32) -> Self {
        Self {
            map,
            stars,
            mods: None,
            clock_rate: None,
        }
    }

    pub fn mods(&mut self, mods: &'a GameMods) -> &mut Self {
        self.mods = Some(mods);

        self
    }

    pub fn clock_rate(&mut self, clock_rate: Option<f64>) -> &mut Self {
        self.clock_rate = clock_rate;

        self
    }

    pub fn keys(mods: u32, cs: f32) -> f32 {
        if (mods & GameModIntermode::OneKey.bits().unwrap()) > 0 {
            1.0
        } else if (mods & GameModIntermode::TwoKeys.bits().unwrap()) > 0 {
            2.0
        } else if (mods & GameModIntermode::ThreeKeys.bits().unwrap()) > 0 {
            3.0
        } else if (mods & GameModIntermode::FourKeys.bits().unwrap()) > 0 {
            4.0
        } else if (mods & GameModIntermode::FiveKeys.bits().unwrap()) > 0 {
            5.0
        } else if (mods & GameModIntermode::SixKeys.bits().unwrap()) > 0 {
            6.0
        } else if (mods & GameModIntermode::SevenKeys.bits().unwrap()) > 0 {
            7.0
        } else if (mods & GameModIntermode::EightKeys.bits().unwrap()) > 0 {
            8.0
        } else if (mods & GameModIntermode::NineKeys.bits().unwrap()) > 0 {
            9.0
        } else {
            round(cs)
        }
    }
}

impl Display for MapInfo<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let mods = self.mods.map(GameMods::to_owned).unwrap_or_default();

        let mut builder = self.map.attributes();

        let clock_rate = self
            .clock_rate
            .or_else(|| self.mods.and_then(GameMods::clock_rate));

        if let Some(clock_rate) = clock_rate {
            builder = builder.clock_rate(clock_rate);
        }

        let mods_bits = mods.bits();
        let attrs = builder.mods(mods).build();

        let clock_rate = attrs.clock_rate;
        let mut sec_drain = self.map.seconds_drain();
        let mut bpm = self.map.bpm();

        if (clock_rate - 1.0).abs() > f64::EPSILON {
            let clock_rate = clock_rate as f32;

            bpm *= clock_rate;
            sec_drain = (sec_drain as f32 / clock_rate) as u32;
        }

        let (cs_key, cs_value) = if self.map.mode() == GameMode::Mania {
            ("Keys", Self::keys(mods_bits, attrs.cs as f32))
        } else {
            ("CS", round(attrs.cs as f32))
        };

        write!(
            f,
            "Length: `{len}` BPM: `{bpm}` Objects: `{objs}`\n\
            {cs_key}: `{cs_value}` AR: `{ar}` OD: `{od}` HP: `{hp}` Stars: `{stars}`",
            len = SecToMinSec::new(sec_drain),
            bpm = round(bpm),
            objs = self.map.n_objects(),
            ar = round(attrs.ar as f32),
            od = round(attrs.od as f32),
            hp = round(attrs.hp as f32),
            stars = round(self.stars),
        )
    }
}

/// Note that all contained indices start at 0.
pub enum PersonalBestIndex {
    /// Found the score in the top100
    FoundScore { idx: usize },
    /// There was a score on the same map with more pp in the top100
    FoundBetter {
        #[allow(unused)]
        idx: usize,
    },
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
        // Note that the index is determined through float comparisons which
        // could result in issues
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
        } else if let Some(top) = top100.get(idx) {
            if score.is_eq(top) {
                return Self::FoundScore { idx };
            } else if let Some((idx, _)) = top100
                .iter()
                .enumerate()
                .skip_while(|(_, top)| top.pp.map_or(true, |pp| pp < score.pp))
                .take_while(|(_, top)| top.pp.is_some_and(|pp| pp <= score.pp))
                .find(|(_, top)| score.is_eq(*top))
            {
                // If multiple scores have the exact same pp as the given score
                // then the initial `idx` might not correspond to it. Hence, if
                // the score at the initial `idx` does not match, we
                // double-check all scores with the same pp.
                return Self::FoundScore { idx };
            }
        }

        let (better, worse) = top100.split_at(idx);

        // A case that's not covered is when there is a score with more pp on
        // the same map with the same mods that has less score than the current
        // score. This should only happen when the top scores haven't been
        // updated yet so the more-pp-but-less-score play is not yet replaced
        // with the new score. Fixes itself over time so it's probably fine to
        // ignore.
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

pub enum MapOrScore {
    Map(MapIdType),
    Score { id: u64, mode: Option<GameMode> },
}

impl MapOrScore {
    /// Try finding the [`MapOrScore`] in the message itself, the message it
    /// replied to, or the forwarded message.
    pub async fn find_in_msg(msg: &Message) -> Option<Self> {
        async fn inner(msg: &Message, depth: usize) -> Option<MapOrScore> {
            if depth > 5 {
                return None;
            }

            if let Some(id) = Context::find_map_id_in_msg(msg).await {
                return Some(MapOrScore::Map(id));
            } else if let Some((id, mode)) = matcher::get_osu_score_id(&msg.content) {
                return Some(MapOrScore::Score { id, mode });
            }

            let reply = msg
                .referenced_message
                .as_deref()
                .filter(|_| msg.kind == MessageType::Reply);

            if let Some(msg) = reply {
                if let opt @ Some(_) = Box::pin(inner(msg, depth + 1)).await {
                    return opt;
                }
            }

            let forwarded = msg.reference.as_ref()?;
            let (channel_id, msg_id) = forwarded.channel_id.zip(forwarded.message_id)?;

            let msg = Context::http()
                .message(channel_id, msg_id)
                .await
                .ok()?
                .model()
                .await
                .ok()?;

            Box::pin(inner(&msg, depth + 1)).await
        }

        inner(msg, 0).await
    }
}
