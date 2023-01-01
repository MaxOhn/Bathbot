use std::{
    array::IntoIter,
    borrow::Cow,
    iter::{self, Copied, Map},
    mem::MaybeUninit,
    path::PathBuf,
    slice::Iter,
};

use eyre::{Result, WrapErr};
use rosu_pp::{CatchPP, DifficultyAttributes, OsuPP, TaikoPP};
use rosu_v2::prelude::{GameMode, GameMods, Grade, Score, ScoreStatistics, UserStatistics};
use time::OffsetDateTime;
use tokio::fs;
use twilight_model::channel::{embed::Embed, Message};

use crate::{
    core::{BotConfig, Context},
    custom_client::{OsuStatsParams, RespektiveTopCount},
    embeds::HitResultFormatter,
    manager::{
        redis::{osu::User, RedisData},
        OsuMap,
    },
    util::{constants::OSU_BASE, matcher, numbers::round, Emote},
};

use super::{numbers::WithComma, ScoreExt};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ModSelection {
    Include(GameMods),
    Exclude(GameMods),
    Exact(GameMods),
}

impl ModSelection {
    pub fn mods(&self) -> GameMods {
        match self {
            Self::Include(m) | Self::Exclude(m) | Self::Exact(m) => *m,
        }
    }

    /// Make sure included or exact mods don't exclude each other e.g. EZHR
    pub fn validate(self) -> Result<(), &'static str> {
        let mods = match self {
            Self::Include(mods) => mods,
            Self::Exclude(_) => return Ok(()),
            Self::Exact(mods) => mods,
        };

        let ezhr = GameMods::Easy | GameMods::HardRock;
        let dtht = GameMods::DoubleTime | GameMods::HalfTime;

        if mods & ezhr == ezhr {
            return Err("Looks like an invalid mod combination, EZ and HR exclude each other");
        }

        if mods & dtht == dtht {
            return Err("Looks like an invalid mod combination, DT and HT exclude each other");
        }

        let mania_mods = GameMods::FadeIn | GameMods::KeyCoop | GameMods::Mirror | GameMods::Random;

        if mods.contains(GameMods::Relax) {
            let excluded = GameMods::Autopilot
                | GameMods::SpunOut
                | GameMods::Autoplay
                | GameMods::Cinema
                | mania_mods;

            if !(mods & excluded).is_empty() || mods.has_key_mod().is_some() {
                let content =
                    "Looks like an invalid mod combination, RX excludes the following mods:\n\
                    AP, SO, FI, MR, RD, 1-9K, Key Coop, Autoplay, and Cinema.";

                return Err(content);
            }
        }

        // * Note: Technically correct but probably unnecessary so might as well save some if's
        // if mods.contains(GameMods::Autopilot) || mods.has_key_mod().is_some() {
        //     let excluded =
        //         GameMods::SpunOut | GameMods::Autoplay | GameMods::Cinema | mania_mods;

        //     if !(mods & excluded).is_empty() {
        //         let content =
        //             "Looks like an invalid mod combination, AP excludes the following mods:\n\
        //             RX, SO, FI, MR, RD, 1-9K, Key Coop, Autoplay, and Cinema";

        //         return Err(content);
        //     }
        // } else if mods.contains(GameMods::SpunOut) {
        //     let excluded = GameMods::Autoplay | GameMods::Cinema | mania_mods;

        //     if !(mods & excluded).is_empty() || mods.has_key_mod().is_some() {
        //         let content =
        //             "Looks like an invalid mod combination, SO excludes the following mods:\n\
        //             RX, AP, FI, MR, RD, 1-9K, Key Coop, Autoplay, and Cinema";

        //         return Err(content);
        //     }
        // } else if mods.contains(GameMods::Autoplay) && mods.contains(GameMods::Cinema) {
        //     let content =
        //         "Looks like an invalid mod combination, Autoplay excludes the following mods:\n\
        //         RX, AP, SO, and Cinema";

        //     return Err(content);
        // }

        Ok(())
    }
}

pub fn flag_url(country_code: &str) -> String {
    // format!("{OSU_BASE}/images/flags/{country_code}.png") // from osu itself but outdated
    format!("https://osuflags.omkserver.nl/{country_code}-256.png") // kelderman
}

pub fn flag_url_svg(country_code: &str) -> String {
    assert_eq!(
        country_code.len(),
        2,
        "country code `{country_code}` is invalid",
    );

    const OFFSET: u32 = 0x1F1A5;
    let bytes = country_code.as_bytes();

    let url = format!(
        "{OSU_BASE}assets/images/flags/{:x}-{:x}.svg",
        bytes[0].to_ascii_uppercase() as u32 + OFFSET,
        bytes[1].to_ascii_uppercase() as u32 + OFFSET
    );

    url
}

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

pub async fn prepare_beatmap_file(ctx: &Context, map_id: u32) -> Result<PathBuf> {
    let mut map_path = BotConfig::get().paths.maps.clone();
    map_path.push(format!("{map_id}.osu"));

    if !map_path.exists() {
        let bytes = ctx
            .client()
            .get_map_file(map_id)
            .await
            .wrap_err("failed to download map")?;

        fs::write(&map_path, &bytes)
            .await
            .wrap_err("failed writing to file")?;

        info!("Downloaded {map_id}.osu successfully");
    }

    Ok(map_path)
}

pub trait ExtractablePp {
    fn extract_pp(&self) -> Vec<f32>;
}

impl ExtractablePp for [Score] {
    fn extract_pp(&self) -> Vec<f32> {
        self.iter().map(|s| s.pp.unwrap_or(0.0)).collect()
    }
}

// Credits to flowabot
/// Extend the list of pps by taking the average difference
/// between 2 values towards the end and create more values
/// based on that difference
pub fn approx_more_pp(pps: &mut Vec<f32>, more: usize) {
    if pps.len() != 100 {
        return;
    }

    let diff = (pps[89] - pps[99]) / 10.0;

    let extension = iter::successors(pps.last().copied(), |pp| {
        let pp = pp - diff;

        (pp > 0.0).then_some(pp)
    });

    pps.extend(extension.take(more));
}

pub trait PpListUtil {
    fn accum_weighted(&self) -> f32;
}

impl PpListUtil for [f32] {
    fn accum_weighted(&self) -> f32 {
        self.iter()
            .copied()
            .zip(0..)
            .fold(0.0, |sum, (pp, i)| sum + pp * 0.95_f32.powi(i))
    }
}

pub trait IntoPpIter {
    type Inner: Iterator<Item = f32> + DoubleEndedIterator + ExactSizeIterator;

    fn into_pps(self) -> PpIter<Self::Inner>;
}

impl<'s> IntoPpIter for &'s [Score] {
    type Inner = Map<Iter<'s, Score>, fn(&Score) -> f32>;

    #[inline]
    fn into_pps(self) -> PpIter<Self::Inner> {
        PpIter {
            inner: self.iter().map(|score| score.pp.unwrap_or(0.0)),
        }
    }
}

impl<'f> IntoPpIter for &'f [f32] {
    type Inner = Copied<Iter<'f, f32>>;

    #[inline]
    fn into_pps(self) -> PpIter<Self::Inner> {
        PpIter {
            inner: self.iter().copied(),
        }
    }
}

pub struct PpIter<I> {
    inner: I,
}

impl<I: Iterator<Item = f32>> Iterator for PpIter<I> {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<I: Iterator<Item = f32> + DoubleEndedIterator> DoubleEndedIterator for PpIter<I> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back()
    }
}

impl<I: Iterator<Item = f32> + ExactSizeIterator> ExactSizeIterator for PpIter<I> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

/// First element: Weighted missing pp to reach goal from start
///
/// Second element: Index of hypothetical pp in pps
pub fn pp_missing(start: f32, goal: f32, pps: impl IntoPpIter) -> (f32, usize) {
    let mut top = start;
    let mut bot = 0.0;

    //     top + x * 0.95^i + bot = goal
    // <=> x = (goal - top - bot) / 0.95^i
    fn calculate_remaining(idx: usize, goal: f32, top: f32, bot: f32) -> (f32, usize) {
        let factor = 0.95_f32.powi(idx as i32);
        let required = (goal - top - bot) / factor;

        (required, idx)
    }

    for (i, last_pp) in pps.into_pps().enumerate().rev() {
        let factor = 0.95_f32.powi(i as i32);
        let term = factor * last_pp;
        let bot_term = term * 0.95;

        if top + bot + bot_term >= goal {
            return calculate_remaining(i + 1, goal, top, bot);
        }

        bot += bot_term;
        top -= term;
    }

    calculate_remaining(0, goal, top, bot)
}

#[derive(Copy, Clone, Debug)]
pub enum MapIdType {
    Map(u32),
    Set(u32),
}

impl MapIdType {
    /// Looks for map or mapset id
    pub fn from_msgs(msgs: &[Message], idx: usize) -> Option<Self> {
        msgs.iter().filter_map(Self::from_msg).nth(idx)
    }

    /// Looks for map or mapset id
    pub fn from_msg(msg: &Message) -> Option<Self> {
        if msg.content.chars().all(|c| c.is_numeric()) {
            return Self::from_embeds(&msg.embeds);
        }

        matcher::get_osu_map_id(&msg.content)
            .map(Self::Map)
            .or_else(|| matcher::get_osu_mapset_id(&msg.content).map(Self::Set))
            .or_else(|| Self::from_embeds(&msg.embeds))
    }

    /// Looks for map or mapset id
    pub fn from_embeds(embeds: &[Embed]) -> Option<Self> {
        embeds.iter().find_map(|embed| {
            let url = embed
                .author
                .as_ref()
                .and_then(|author| author.url.as_deref());

            url.and_then(matcher::get_osu_map_id)
                .map(Self::Map)
                .or_else(|| url.and_then(matcher::get_osu_mapset_id).map(Self::Set))
                .or_else(|| {
                    embed
                        .url
                        .as_deref()
                        .and_then(matcher::get_osu_map_id)
                        .map(Self::Map)
                })
                .or_else(|| {
                    embed
                        .url
                        .as_deref()
                        .and_then(matcher::get_osu_mapset_id)
                        .map(Self::Set)
                })
        })
    }

    /// Only looks for map id
    pub fn map_from_msgs(msgs: &[Message], idx: usize) -> Option<u32> {
        msgs.iter().filter_map(Self::map_from_msg).nth(idx)
    }

    /// Only looks for map id
    pub fn map_from_msg(msg: &Message) -> Option<u32> {
        if msg.content.chars().all(|c| c.is_numeric()) {
            return Self::map_from_embeds(&msg.embeds);
        }

        matcher::get_osu_map_id(&msg.content).or_else(|| Self::map_from_embeds(&msg.embeds))
    }

    /// Only looks for map id
    pub fn map_from_embeds(embeds: &[Embed]) -> Option<u32> {
        embeds.iter().find_map(|embed| {
            embed
                .author
                .as_ref()
                .and_then(|author| author.url.as_deref())
                .and_then(matcher::get_osu_map_id)
                .or_else(|| embed.url.as_deref().and_then(matcher::get_osu_map_id))
        })
    }
}

// Credits to https://github.com/RoanH/osu-BonusPP/blob/master/BonusPP/src/me/roan/bonuspp/BonusPP.java#L202
pub struct BonusPP {
    pp: f32,
    ys: [f32; 100],
    len: usize,

    sum_x: f32,
    avg_x: f32,
    avg_y: f32,
}

impl BonusPP {
    const MAX: f32 = 416.67;

    pub fn new() -> Self {
        Self {
            pp: 0.0,
            ys: [0.0; 100],
            len: 0,

            sum_x: 0.0,
            avg_x: 0.0,
            avg_y: 0.0,
        }
    }

    pub fn update(&mut self, weighted_pp: f32, idx: usize) {
        self.pp += weighted_pp;
        self.ys[idx] = weighted_pp.log(100.0);
        self.len += 1;

        let n = idx as f32 + 1.0;
        let weight = n.ln_1p();

        self.sum_x += weight;
        self.avg_x += n * weight;
        self.avg_y += self.ys[idx] * weight;
    }

    pub fn calculate(self, stats: &UserStatistics) -> f32 {
        let BonusPP {
            mut pp,
            len,
            ys,
            sum_x,
            mut avg_x,
            mut avg_y,
        } = self;

        if stats.pp.abs() < f32::EPSILON {
            let counts = &stats.grade_counts;
            let sum = counts.ssh + counts.ss + counts.sh + counts.s + counts.a;

            return round(Self::MAX * (1.0 - 0.9994_f32.powi(sum)));
        } else if self.len < 100 {
            return round(stats.pp - pp);
        }

        avg_x /= sum_x;
        avg_y /= sum_x;

        let mut sum_xy = 0.0;
        let mut sum_x2 = 0.0;

        for n in 1..=len {
            let diff_x = n as f32 - avg_x;
            let ln_n = (n as f32).ln_1p();

            sum_xy += diff_x * (ys[n - 1] - avg_y) * ln_n;
            sum_x2 += diff_x * diff_x * ln_n;
        }

        let xy = sum_xy / sum_x;
        let x2 = sum_x2 / sum_x;

        let m = xy / x2;
        let b = avg_y - (xy / x2) * avg_x;

        for n in 100..=stats.playcount {
            let val = 100.0_f32.powf(m * n as f32 + b);

            if val <= 0.0 {
                break;
            }

            pp += val;
        }

        round(stats.pp - pp).clamp(0.0, Self::MAX)
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum AttributeKind {
    Ar,
    Cs,
    Hp,
    Od,
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
pub struct ScoreSlim {
    pub accuracy: f32,
    pub ended_at: OffsetDateTime,
    pub grade: Grade,
    pub max_combo: u32,
    pub mode: GameMode,
    pub mods: GameMods,
    pub pp: f32,
    pub score: u32,
    pub score_id: Option<u64>,
    pub statistics: ScoreStatistics,
}

impl ScoreSlim {
    pub fn new(score: Score, pp: f32) -> Self {
        Self {
            accuracy: score.accuracy,
            ended_at: score.ended_at,
            grade: score.grade,
            max_combo: score.max_combo,
            mode: score.mode,
            mods: score.mods,
            pp,
            score: score.score,
            score_id: score.score_id,
            statistics: score.statistics,
        }
    }

    pub fn total_hits(&self) -> u32 {
        self.statistics.total_hits(self.mode)
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
                    count_miss: statistics.count_miss,
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
                    count_miss: statistics.count_miss,
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
                    count_miss: statistics.count_miss,
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

pub fn calculate_grade(mode: GameMode, mods: GameMods, stats: &ScoreStatistics) -> Grade {
    match mode {
        GameMode::Osu => osu_grade(mods, stats),
        GameMode::Taiko => taiko_grade(mods, stats),
        GameMode::Catch => catch_grade(mods, stats),
        GameMode::Mania => mania_grade(mods, stats),
    }
}

fn osu_grade(mods: GameMods, stats: &ScoreStatistics) -> Grade {
    let passed_objects = stats.total_hits(GameMode::Osu);

    if stats.count_300 == passed_objects {
        return if mods.contains(GameMods::Hidden) || mods.contains(GameMods::Flashlight) {
            Grade::XH
        } else {
            Grade::X
        };
    }

    let ratio300 = stats.count_300 as f32 / passed_objects as f32;
    let ratio50 = stats.count_50 as f32 / passed_objects as f32;

    if ratio300 > 0.9 && ratio50 < 0.01 && stats.count_miss == 0 {
        if mods.intersects(GameMods::Hidden | GameMods::Flashlight) {
            Grade::SH
        } else {
            Grade::S
        }
    } else if ratio300 > 0.9 || (ratio300 > 0.8 && stats.count_miss == 0) {
        Grade::A
    } else if ratio300 > 0.8 || (ratio300 > 0.7 && stats.count_miss == 0) {
        Grade::B
    } else if ratio300 > 0.6 {
        Grade::C
    } else {
        Grade::D
    }
}

fn taiko_grade(mods: GameMods, stats: &ScoreStatistics) -> Grade {
    let passed_objects = stats.total_hits(GameMode::Taiko);
    let count_300 = stats.count_300;

    if count_300 == passed_objects {
        return if mods.intersects(GameMods::Hidden | GameMods::Flashlight) {
            Grade::XH
        } else {
            Grade::X
        };
    }

    let ratio300 = count_300 as f32 / passed_objects as f32;
    let count_miss = stats.count_miss;

    if ratio300 > 0.9 && count_miss == 0 {
        if mods.intersects(GameMods::Hidden | GameMods::Flashlight) {
            Grade::SH
        } else {
            Grade::S
        }
    } else if ratio300 > 0.9 || (ratio300 > 0.8 && count_miss == 0) {
        Grade::A
    } else if ratio300 > 0.8 || (ratio300 > 0.7 && count_miss == 0) {
        Grade::B
    } else if ratio300 > 0.6 {
        Grade::C
    } else {
        Grade::D
    }
}

fn catch_grade(mods: GameMods, stats: &ScoreStatistics) -> Grade {
    let acc = stats.accuracy(GameMode::Catch);

    if (100.0 - acc).abs() <= std::f32::EPSILON {
        if mods.intersects(GameMods::Hidden | GameMods::Flashlight) {
            Grade::XH
        } else {
            Grade::X
        }
    } else if acc > 98.0 {
        if mods.intersects(GameMods::Hidden | GameMods::Flashlight) {
            Grade::SH
        } else {
            Grade::S
        }
    } else if acc > 94.0 {
        Grade::A
    } else if acc > 90.0 {
        Grade::B
    } else if acc > 85.0 {
        Grade::C
    } else {
        Grade::D
    }
}

fn mania_grade(mods: GameMods, stats: &ScoreStatistics) -> Grade {
    let passed_objects = stats.total_hits(GameMode::Mania);

    if stats.count_geki == passed_objects {
        return if mods.intersects(GameMods::Hidden | GameMods::Flashlight) {
            Grade::XH
        } else {
            Grade::X
        };
    }

    let acc = stats.accuracy(GameMode::Mania);

    if acc > 95.0 {
        if mods.intersects(GameMods::Hidden | GameMods::Flashlight) {
            Grade::SH
        } else {
            Grade::S
        }
    } else if acc > 90.0 {
        Grade::A
    } else if acc > 80.0 {
        Grade::B
    } else if acc > 70.0 {
        Grade::C
    } else {
        Grade::D
    }
}
