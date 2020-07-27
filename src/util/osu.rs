use crate::{
    core::CachedEmoji,
    util::{
        constants::{emotes::*, DEV_GUILD_ID, OSU_BASE},
        error::MapDownloadError,
        matcher,
    },
    BotResult, Context,
};

use rosu::models::{Beatmap, GameMode, GameMods, Grade, Score};
use std::{env, path::Path, sync::Arc};
use tokio::{fs::File, io::AsyncWriteExt};
use twilight::model::{channel::Message, id::EmojiId};

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

pub fn grade_emote(grade: Grade, ctx: &Context) -> Arc<CachedEmoji> {
    let emoji_id = match grade {
        Grade::XH => EmojiId(EMOTE_XH_ID),
        Grade::X => EmojiId(EMOTE_X_ID),
        Grade::SH => EmojiId(EMOTE_SH_ID),
        Grade::S => EmojiId(EMOTE_S_ID),
        Grade::A => EmojiId(EMOTE_A_ID),
        Grade::B => EmojiId(EMOTE_B_ID),
        Grade::C => EmojiId(EMOTE_C_ID),
        Grade::D => EmojiId(EMOTE_D_ID),
        Grade::F => EmojiId(EMOTE_F_ID),
    };
    ctx.cache
        .emoji
        .get(&emoji_id)
        .unwrap_or_else(|| panic!("Emote {} not found", emoji_id.0))
        .value()
        .clone()
}

pub async fn prepare_beatmap_file(map_id: u32) -> Result<String, MapDownloadError> {
    let map_path = format!(
        "{base}{id}.osu",
        base = env::var("BEATMAP_PATH").map_err(|_| MapDownloadError::NoEnv)?,
        id = map_id
    );
    if !Path::new(&map_path).exists() {
        let mut file = File::create(&map_path).await?;
        let download_url = format!("{}web/maps/{}", OSU_BASE, map_id);
        let content = reqwest::get(&download_url).await?.bytes().await?;
        file.write_all(&content).await?;
        info!("Downloaded {}.osu successfully", map_id);
    }
    Ok(map_path)
}

// pub fn simulate_score(score: &mut Score, map: &Beatmap, args: SimulateArgs) {
//     if let Some((mods, selection)) = args.mods {
//         if selection == ModSelection::Exact || selection == ModSelection::Includes {
//             score.enabled_mods = mods;
//         }
//     }
//     match map.mode {
//         GameMode::STD => {
//             let acc = args.acc.unwrap_or_else(|| {
//                 let acc = score.accuracy(map.mode);
//                 if acc.is_nan() {
//                     100.0
//                 } else {
//                     acc
//                 }
//             }) / 100.0;
//             let n50 = args.n50.unwrap_or(0);
//             let n100 = args.n100.unwrap_or(0);
//             let miss = args.miss.unwrap_or(0);
//             let total_objects = map.count_objects();
//             let combo = args
//                 .combo
//                 .or_else(|| map.max_combo)
//                 .unwrap_or_else(|| panic!("Combo of args / beatmap not found"));
//             if n50 > 0 || n100 > 0 {
//                 score.count300 = total_objects - n100.max(0) - n50.max(0) - miss;
//                 score.count100 = n100;
//                 score.count50 = n50;
//             } else {
//                 let target_total = (acc * 6.0 * total_objects as f32) as u32;
//                 let delta = target_total + miss - total_objects;
//                 score.count300 = (delta as f32 / 5.0) as u32;
//                 score.count100 = delta % 5;
//                 score.count50 = total_objects - score.count300 - score.count100 - miss;
//             }
//             score.count_miss = miss;
//             score.max_combo = combo;
//             score.recalculate_grade(GameMode::STD, Some(acc * 100.0));
//         }
//         GameMode::MNA => {
//             score.max_combo = 0;
//             score.score = args.score.unwrap_or(1_000_000);
//             score.count_geki = map.count_objects();
//             score.count300 = 0;
//             score.count_katu = 0;
//             score.count100 = 0;
//             score.count50 = 0;
//             score.count_miss = 0;
//             score.grade = if score
//                 .enabled_mods
//                 .intersects(GameMods::Flashlight | GameMods::Hidden)
//             {
//                 Grade::XH
//             } else {
//                 Grade::X
//             };
//         }
//         _ => panic!("Can only simulate STD and MNA scores, not {:?}", map.mode,),
//     }
// }

pub fn unchoke_score(score: &mut Score, map: &Beatmap) {
    score.pp = None;
    match map.mode {
        GameMode::STD => {
            let max_combo = map
                .max_combo
                .unwrap_or_else(|| panic!("Max combo of beatmap not found"));
            if score.max_combo == max_combo {
                return;
            }
            let total_objects = map.count_objects();
            let passed_objects = score.total_hits(GameMode::STD);
            score.count300 += total_objects.saturating_sub(passed_objects);
            let count_hits = total_objects - score.count_miss;
            let ratio = score.count300 as f32 / count_hits as f32;
            let new100s = (ratio * score.count_miss as f32) as u32;
            score.count100 += new100s;
            score.count300 += score.count_miss.saturating_sub(new100s);
            score.max_combo = max_combo;
            score.count_miss = 0;
            score.recalculate_grade(GameMode::STD, None);
        }
        GameMode::MNA => {
            score.max_combo = 0;
            score.score = 1_000_000;
            score.count_geki = map.count_objects();
            score.count300 = 0;
            score.count_katu = 0;
            score.count100 = 0;
            score.count50 = 0;
            score.count_miss = 0;
            score.grade = if score.enabled_mods.contains(GameMods::Hidden) {
                Grade::XH
            } else {
                Grade::X
            };
        }
        _ => panic!("Can only unchoke STD and MNA scores, not {:?}", map.mode,),
    }
}

/// First element: Weighted missing pp to reach goal from start
///
/// Second element: Index of hypothetical pp in scores
pub fn pp_missing(start: f32, goal: f32, scores: &[Score]) -> (f32, usize) {
    let pp_values: Vec<f32> = scores.iter().map(|score| score.pp.unwrap()).collect();
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

pub async fn map_id_from_history(ctx: &Context, msgs: Vec<Message>) -> Option<MapIdType> {
    for msg in msgs {
        if !ctx.is_own(&msg) {
            continue;
        }
        for embed in msg.embeds {
            let url = embed.author.and_then(|author| author.url);
            if let Some(id) = url.as_deref().and_then(matcher::get_osu_map_id) {
                return Some(id);
            }
            if let Some(id) = url.as_deref().and_then(matcher::get_osu_mapset_id) {
                return Some(id);
            }
            if embed.fields.is_empty() {
                continue;
            }
            let url = embed.url.as_deref();
            if let Some(id) = url.and_then(matcher::get_osu_map_id) {
                return Some(id);
            }
            if let Some(id) = url.and_then(matcher::get_osu_mapset_id) {
                return Some(id);
            }
        }
    }
    None
}

pub enum MapIdType {
    Map(u32),
    Mapset(u32),
}

impl MapIdType {
    pub fn id(&self) -> u32 {
        match self {
            Self::Map(id) | Self::Mapset(id) => *id,
        }
    }
}
