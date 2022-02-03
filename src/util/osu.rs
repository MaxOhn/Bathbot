use super::ExponentialBackoff;
use crate::{
    error::MapDownloadError,
    util::{constants::OSU_BASE, matcher, numbers::round, BeatmapExt, Emote, ScoreExt},
    CONFIG,
};

use bytes::Bytes;
use rosu_v2::prelude::{Beatmap, GameMode, GameMods, Grade, Score, UserStatistics};
use std::borrow::Cow;
use tokio::{fs::File, io::AsyncWriteExt, time::sleep};
use twilight_model::channel::{embed::Embed, Message};

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
}

pub fn flag_url(country_code: &str) -> String {
    // format!("{}/images/flags/{}.png", OSU_BASE, country_code) // from osu itself but outdated
    format!("https://osuflags.omkserver.nl/{country_code}-256.png") // kelderman
}

#[allow(dead_code)]
pub fn flag_url_svg(country_code: &str) -> String {
    assert_eq!(
        country_code.len(),
        2,
        "country code `{}` is invalid",
        country_code
    );

    const OFFSET: u32 = 0x1F1A5;
    let bytes = country_code.as_bytes();

    let url = format!(
        "{}assets/images/flags/{:x}-{:x}.svg",
        OSU_BASE,
        bytes[0].to_ascii_uppercase() as u32 + OFFSET,
        bytes[1].to_ascii_uppercase() as u32 + OFFSET
    );

    url
}

pub fn grade_emote(grade: Grade) -> &'static str {
    CONFIG.get().unwrap().grade(grade)
}

pub fn mode_emote(mode: GameMode) -> Cow<'static, str> {
    let emote = match mode {
        GameMode::STD => Emote::Std,
        GameMode::TKO => Emote::Tko,
        GameMode::CTB => Emote::Ctb,
        GameMode::MNA => Emote::Mna,
    };

    emote.text()
}

pub fn grade_completion_mods(score: &dyn ScoreExt, map: &Beatmap) -> Cow<'static, str> {
    let mode = map.mode();
    let grade = CONFIG.get().unwrap().grade(score.grade(mode));
    let mods = score.mods();

    match (
        mods.is_empty(),
        score.grade(mode) == Grade::F && mode != GameMode::CTB,
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

pub async fn prepare_beatmap_file(map_id: u32) -> Result<String, MapDownloadError> {
    let mut map_path = CONFIG.get().unwrap().map_path.clone();
    map_path.push(format!("{map_id}.osu"));

    if !map_path.exists() {
        let content = request_beatmap_file(map_id).await?;
        let mut file = File::create(&map_path).await?;
        file.write_all(&content).await?;
        info!("Downloaded {map_id}.osu successfully");
    }

    let map_path = map_path
        .into_os_string()
        .into_string()
        .expect("map_path OsString is no valid String");

    Ok(map_path)
}

async fn request_beatmap_file(map_id: u32) -> Result<Bytes, MapDownloadError> {
    let url = format!("{OSU_BASE}osu/{map_id}");
    let mut content = reqwest::get(&url).await?.bytes().await?;

    if content.len() >= 6 && &content.slice(0..6)[..] != b"<html>" {
        return Ok(content);
    }

    // 1s - 2s - 4s - 8s - 10s - 10s - 10s - 10s - 10s - 10s - Give up
    let backoff = ExponentialBackoff::new(2).factor(500).max_delay(10_000);

    for (duration, i) in backoff.take(10).zip(1..) {
        debug!("Request beatmap retry attempt #{i} | Backoff {duration:?}",);
        sleep(duration).await;

        content = reqwest::get(&url).await?.bytes().await?;

        if content.len() >= 6 && &content.slice(0..6)[..] != b"<html>" {
            return Ok(content);
        }
    }

    (content.len() >= 6 && &content.slice(0..6)[..] != b"<html>")
        .then(|| content)
        .ok_or(MapDownloadError::RetryLimit(map_id))
}

macro_rules! pp {
    ($scores:ident[$idx:expr]) => {
        $scores.get($idx).and_then(|s| s.pp).unwrap_or(0.0)
    };
}

/// First element: Weighted missing pp to reach goal from start
///
/// Second element: Index of hypothetical pp in scores
pub fn pp_missing(start: f32, goal: f32, scores: &[Score]) -> (f32, usize) {
    let size: usize = scores.len();
    let mut idx: usize = size - 1;
    let mut factor: f32 = 0.95_f32.powi(idx as i32);
    let mut top: f32 = start;
    let mut bot: f32 = 0.0;
    let mut current: f32 = pp!(scores[idx]);

    while top + bot < goal {
        top -= current * factor;

        if idx == 0 {
            break;
        }

        current = pp!(scores[idx - 1]);
        bot += current * factor;
        factor /= 0.95;
        idx -= 1;
    }

    let mut required: f32 = goal - top - bot;

    if top + bot >= goal {
        factor *= 0.95;
        required = (required + factor * pp!(scores[idx])) / factor;
        idx += 1;
    }

    idx += 1;

    if size < 100 {
        required -= pp!(scores[size - 1]) * 0.95_f32.powi(size as i32 - 1);
    }

    (required, idx)
}

pub fn map_id_from_history(msgs: &[Message]) -> Option<MapIdType> {
    msgs.iter().find_map(map_id_from_msg)
}

pub fn map_id_from_msg(msg: &Message) -> Option<MapIdType> {
    if msg.content.chars().all(|c| c.is_numeric()) {
        return check_embeds_for_map_id(&msg.embeds);
    }

    matcher::get_osu_map_id(&msg.content)
        .or_else(|| matcher::get_osu_mapset_id(&msg.content))
        .or_else(|| check_embeds_for_map_id(&msg.embeds))
}

fn check_embeds_for_map_id(embeds: &[Embed]) -> Option<MapIdType> {
    embeds.iter().find_map(|embed| {
        let url = embed
            .author
            .as_ref()
            .and_then(|author| author.url.as_deref());

        url.and_then(matcher::get_osu_map_id)
            .or_else(|| url.and_then(matcher::get_osu_mapset_id))
            .or_else(|| embed.url.as_deref().and_then(matcher::get_osu_map_id))
            .or_else(|| embed.url.as_deref().and_then(matcher::get_osu_mapset_id))
    })
}

#[derive(Copy, Clone, Debug)]
pub enum MapIdType {
    Map(u32),
    Set(u32),
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

        round(stats.pp - pp).min(Self::MAX)
    }
}
