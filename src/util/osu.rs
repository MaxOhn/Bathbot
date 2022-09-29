use std::{
    borrow::Cow,
    iter::{self, Copied, Map},
    path::PathBuf,
    slice::Iter,
};

use eyre::{Result, WrapErr};
use rosu_v2::prelude::{Beatmap, GameMode, GameMods, Grade, Score, UserStatistics};
use time::OffsetDateTime;
use tokio::fs;
use twilight_model::channel::{embed::Embed, Message};

use crate::{
    core::{BotConfig, Context},
    custom_client::OsuTrackerCountryScore,
    util::{constants::OSU_BASE, matcher, numbers::round, BeatmapExt, Emote, ScoreExt},
};

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

pub fn grade_completion_mods(score: &dyn ScoreExt, map: &Beatmap) -> Cow<'static, str> {
    let mode = map.mode();
    let grade = BotConfig::get().grade(score.grade(mode));
    let mods = score.mods();

    match (
        mods.is_empty(),
        score.grade(mode) == Grade::F && mode != GameMode::Catch,
    ) {
        (true, true) => format!("{grade} ({}%)", completion(score, map)).into(),
        (false, true) => format!("{grade} ({}%) +{mods}", completion(score, map)).into(),
        (true, false) => grade.into(),
        (false, false) => format!("{grade} +{mods}").into(),
    }
}

fn completion(score: &dyn ScoreExt, map: &Beatmap) -> u32 {
    let passed = score.hits(map.mode() as u8);
    let total = map.count_objects();

    100 * passed / total
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

pub trait SortableScore {
    fn acc(&self) -> f32;
    fn bpm(&self) -> f32;
    fn ended_at(&self) -> OffsetDateTime;
    fn map_id(&self) -> u32;
    fn mapset_id(&self) -> u32;
    fn max_combo(&self) -> u32;
    fn mode(&self) -> GameMode;
    fn mods(&self) -> GameMods;
    fn n_misses(&self) -> u32;
    fn pp(&self) -> Option<f32>;
    fn score(&self) -> u32;
    fn score_id(&self) -> u64;
    fn seconds_drain(&self) -> u32;
    fn stars(&self) -> f32;
    fn total_hits_sort(&self) -> u32;
}

impl SortableScore for Score {
    fn acc(&self) -> f32 {
        self.accuracy
    }

    fn bpm(&self) -> f32 {
        self.map.as_ref().map_or(0.0, |map| map.bpm)
    }

    fn ended_at(&self) -> OffsetDateTime {
        self.ended_at
    }

    fn map_id(&self) -> u32 {
        self.map.as_ref().map_or(0, |map| map.map_id)
    }

    fn mapset_id(&self) -> u32 {
        self.mapset.as_ref().map_or(0, |mapset| mapset.mapset_id)
    }

    fn max_combo(&self) -> u32 {
        self.max_combo
    }

    fn mode(&self) -> GameMode {
        self.mode
    }

    fn mods(&self) -> GameMods {
        self.mods
    }

    fn n_misses(&self) -> u32 {
        self.statistics.count_miss
    }

    fn pp(&self) -> Option<f32> {
        self.pp
    }

    fn score(&self) -> u32 {
        self.score
    }

    fn score_id(&self) -> u64 {
        self.score_id.unwrap_or(0)
    }

    fn seconds_drain(&self) -> u32 {
        self.map.as_ref().map_or(0, |map| map.seconds_drain)
    }

    fn stars(&self) -> f32 {
        self.map.as_ref().map_or(0.0, |map| map.stars)
    }

    fn total_hits_sort(&self) -> u32 {
        self.total_hits()
    }
}

macro_rules! impl_sortable_score_tuple {
    (($($ty:ty),*) => $idx:tt) => {
        impl SortableScore for ($($ty),*) {
            fn acc(&self) -> f32 {
                SortableScore::acc(&self.$idx)
            }

            fn bpm(&self) -> f32 {
                SortableScore::bpm(&self.$idx)
            }

            fn ended_at(&self) -> OffsetDateTime {
                SortableScore::ended_at(&self.$idx)
            }

            fn map_id(&self) -> u32 {
                SortableScore::map_id(&self.$idx)
            }

            fn mapset_id(&self) -> u32 {
                SortableScore::mapset_id(&self.$idx)
            }

            fn max_combo(&self) -> u32 {
                SortableScore::max_combo(&self.$idx)
            }

            fn mode(&self) -> GameMode {
                SortableScore::mode(&self.$idx)
            }

            fn mods(&self) -> GameMods {
                SortableScore::mods(&self.$idx)
            }

            fn n_misses(&self) -> u32 {
                SortableScore::n_misses(&self.$idx)
            }

            fn pp(&self) -> Option<f32> {
                SortableScore::pp(&self.$idx)
            }

            fn score(&self) -> u32 {
                SortableScore::score(&self.$idx)
            }

            fn score_id(&self) -> u64 {
                SortableScore::score_id(&self.$idx)
            }

            fn seconds_drain(&self) -> u32 {
                SortableScore::seconds_drain(&self.$idx)
            }

            fn stars(&self) -> f32 {
                SortableScore::stars(&self.1)
            }

            fn total_hits_sort(&self) -> u32 {
                SortableScore::total_hits_sort(&self.$idx)
            }
        }
    };
}

impl_sortable_score_tuple!((usize, Score) => 1);
impl_sortable_score_tuple!((usize, Score, Option<f32>) => 1);

impl SortableScore for (OsuTrackerCountryScore, usize) {
    fn acc(&self) -> f32 {
        self.0.acc
    }

    fn bpm(&self) -> f32 {
        panic!("can't sort by bpm")
    }

    fn ended_at(&self) -> OffsetDateTime {
        self.0.ended_at
    }

    fn map_id(&self) -> u32 {
        self.0.map_id
    }

    fn mapset_id(&self) -> u32 {
        self.0.mapset_id
    }

    fn max_combo(&self) -> u32 {
        panic!("can't sort by combo")
    }

    fn mode(&self) -> GameMode {
        GameMode::Osu
    }

    fn mods(&self) -> GameMods {
        self.0.mods
    }

    fn n_misses(&self) -> u32 {
        self.0.n_misses
    }

    fn pp(&self) -> Option<f32> {
        Some(self.0.pp)
    }

    fn score(&self) -> u32 {
        panic!("can't sort by score")
    }

    fn score_id(&self) -> u64 {
        panic!("can't sort with score id")
    }

    fn seconds_drain(&self) -> u32 {
        self.0.seconds_total
    }

    fn stars(&self) -> f32 {
        panic!("can't sort by stars")
    }

    fn total_hits_sort(&self) -> u32 {
        self.0.n_misses + 1
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum AttributeKind {
    Ar,
    Cs,
    Hp,
    Od,
}
