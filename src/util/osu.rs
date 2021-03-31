use crate::{
    util::{constants::OSU_BASE, error::MapDownloadError, matcher, BeatmapExt, ScoreExt},
    CONFIG,
};

use rosu_v2::prelude::{Beatmap, GameMode, GameMods, Grade, Score};
use tokio::{
    fs::File,
    io::AsyncWriteExt,
    time::{sleep, Duration},
};
use twilight_cache_inmemory::model::CachedMessage;
use twilight_model::channel::{embed::Embed, Message};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ModSelection {
    Include(GameMods),
    Exclude(GameMods),
    Exact(GameMods),
}

impl ModSelection {
    #[inline]
    pub fn mods(&self) -> GameMods {
        match self {
            Self::Include(m) | Self::Exclude(m) | Self::Exact(m) => *m,
        }
    }
}

#[inline]
pub fn grade_emote(grade: Grade) -> String {
    CONFIG.get().unwrap().grade(grade).to_owned()
}

#[inline]
pub fn mode_emote(mode: GameMode) -> String {
    CONFIG.get().unwrap().modes[&mode].to_owned()
}

pub fn grade_completion_mods(score: &impl ScoreExt, map: &Beatmap) -> String {
    let mode = map.mode();
    let grade = CONFIG.get().unwrap().grade(score.grade(mode));
    let mods = score.mods();

    match (
        mods.is_empty(),
        score.grade(mode) == Grade::F && mode != GameMode::CTB,
    ) {
        (true, true) => format!("{} ({}%)", grade, completion(score, map)),
        (false, true) => format!("{} ({}%) +{}", grade, completion(score, map), mods),
        (true, false) => grade.to_owned(),
        (false, false) => format!("{} +{}", grade, mods),
    }
}

#[inline]
fn completion(score: &impl ScoreExt, map: &Beatmap) -> u32 {
    let passed = score.hits(map.mode() as u8);
    let total = map.count_objects();

    100 * passed / total
}

pub async fn prepare_beatmap_file(map_id: u32) -> Result<String, MapDownloadError> {
    let mut map_path = CONFIG.get().unwrap().map_path.clone();
    map_path.push(format!("{}.osu", map_id));

    if !map_path.exists() {
        let download_url = format!("{}osu/{}", OSU_BASE, map_id);
        let mut content;
        let mut delay = 500;
        let mut attempts = 10;

        while {
            content = reqwest::get(&download_url).await?.bytes().await?;
            (content.len() < 6 || &content.slice(0..6)[..] == b"<html>") && attempts > 0
        } {
            info!("Received invalid {}.osu, {}ms backoff", map_id, delay);
            sleep(Duration::from_millis(delay)).await;

            delay = (delay * 2).min(10_000);
            attempts -= 1;
        }

        if content.len() < 6 || &content.slice(0..6)[..] == b"<html>" {
            return Err(MapDownloadError::Content(map_id));
        }

        let mut file = File::create(&map_path).await?;
        file.write_all(&content).await?;
        info!("Downloaded {}.osu successfully", map_id);
    }

    Ok(map_path.to_str().unwrap().to_owned())
}

/// First element: Weighted missing pp to reach goal from start
///
/// Second element: Index of hypothetical pp in scores
pub fn pp_missing(start: f32, goal: f32, scores: &[Score]) -> (f32, usize) {
    let pp_values: Vec<f32> = scores.iter().filter_map(|score| score.pp).collect();
    let size: usize = pp_values.len();
    let mut idx: usize = size - 1;
    let mut factor: f32 = 0.95_f32.powi(idx as i32);
    let mut top: f32 = start;
    let mut bot: f32 = 0.0;
    let mut current: f32 = pp_values[idx];

    while top + bot < goal {
        top -= current * factor;

        if idx == 0 {
            break;
        }

        current = pp_values[idx - 1];
        bot += current * factor;
        factor /= 0.95;
        idx -= 1;
    }

    let mut required: f32 = goal - top - bot;

    if top + bot >= goal {
        factor *= 0.95;
        required = (required + factor * pp_values[idx]) / factor;
        idx += 1;
    }

    idx += 1;

    if size < 100 {
        required -= pp_values[size - 1] * 0.95_f32.powi(size as i32 - 1);
    }

    (required, idx)
}

pub fn map_id_from_history(msgs: Vec<Message>) -> Option<MapIdType> {
    msgs.into_iter().find_map(|msg| {
        matcher::get_osu_map_id(&msg.content).or_else(|| {
            matcher::get_osu_mapset_id(&msg.content)
                .or_else(|| check_embeds_for_map_id(&msg.embeds))
        })
    })
}

#[inline]
pub fn cached_message_extract(msg: &CachedMessage) -> Option<MapIdType> {
    check_embeds_for_map_id(&msg.embeds)
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

#[derive(Debug, Clone, Copy)]
pub enum MapIdType {
    Map(u32),
    Set(u32),
}

impl MapIdType {
    #[inline]
    pub fn id(&self) -> u32 {
        match self {
            Self::Map(id) | Self::Set(id) => *id,
        }
    }
}
