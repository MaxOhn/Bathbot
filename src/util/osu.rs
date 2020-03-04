use crate::{
    arguments::{ModSelection, SimulateArgs},
    util::globals::{emotes::*, DEV_GUILD_ID, HOMEPAGE},
    Error,
};
use rosu::models::{Beatmap, GameMod, GameMode, Grade, Score};
use serenity::{
    cache::CacheRwLock,
    model::{
        guild::Emoji,
        id::{EmojiId, GuildId},
    },
};
use std::{env, fs::File, io::Write, path::Path};
use tokio::runtime::Runtime;

pub fn prepare_beatmap_file(map_id: u32) -> Result<String, Error> {
    let map_path = format!(
        "{base}{id}.osu",
        base = if cfg!(debug_assertions) {
            env::var("BEATMAP_PATH_DEV")
        } else {
            env::var("BEATMAP_PATH_RELEASE")
        }?,
        id = map_id
    );
    if !Path::new(&map_path).exists() {
        let mut file = File::create(&map_path)?;
        let download_url = format!("{}web/maps/{}", HOMEPAGE, map_id);
        let mut rt = Runtime::new().unwrap();
        let content = rt.block_on(async { reqwest::get(&download_url).await?.text().await })?;
        file.write_all(content.as_bytes())?;
        debug!("Downloaded {}.osu successfully", map_id);
    }
    Ok(map_path)
}

pub fn grade_emote(grade: Grade, cache: CacheRwLock) -> Emoji {
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
    let guild = GuildId(DEV_GUILD_ID).to_guild_cached(cache);
    guild
        .unwrap()
        .read()
        .emojis
        .get(&emoji_id)
        .unwrap_or_else(|| {
            panic!(
                "Could not find emote with id {} for grade {}",
                emoji_id.0, grade
            )
        })
        .clone()
}

pub fn simulate_score(score: &mut Score, map: &Beatmap, args: SimulateArgs) {
    if let Some((mods, selection)) = args.mods {
        if selection == ModSelection::Exact || selection == ModSelection::Includes {
            score.enabled_mods = mods;
        }
    }
    match map.mode {
        GameMode::STD => {
            let acc = args.acc.unwrap_or(0.0) / 100.0;
            let n50 = args.n50.unwrap_or(0);
            let n100 = args.n100.unwrap_or(0);
            let miss = args.miss.unwrap_or(0);
            let total_objects = map.count_objects();
            let combo = args
                .combo
                .or_else(|| map.max_combo)
                .unwrap_or_else(|| panic!("Combo of args / beatmap not found"));
            if n50 > 0 || n100 > 0 {
                score.count300 = total_objects - n100.max(0) - n50.max(0) - miss;
                score.count100 = n100;
                score.count50 = n50;
            } else {
                let target_total = (acc * 6.0 * total_objects as f32) as u32;
                let delta = target_total + miss - total_objects;
                score.count300 = (delta as f32 / 5.0) as u32;
                score.count100 = delta % 5;
                score.count50 = total_objects - score.count300 - score.count100 - miss;
            }
            score.count_miss = miss;
            score.max_combo = combo;
            score.recalculate_grade(GameMode::STD, Some(acc * 100.0));
        }
        GameMode::MNA => {
            score.max_combo = 0;
            score.score = args.score.unwrap_or(1_000_000);
            score.count_geki = map.count_objects();
            score.count300 = 0;
            score.count_katu = 0;
            score.count100 = 0;
            score.count50 = 0;
            score.count_miss = 0;
            score.grade = if score.enabled_mods.contains(&GameMod::Hidden) {
                Grade::XH
            } else {
                Grade::X
            };
        }
        GameMode::TKO => {
            let acc = args.acc.unwrap_or(0.0) / 100.0;
            let n100 = args.n100.unwrap_or(0);
            let miss = args.miss.unwrap_or(0);
            let total_objects = map.count_objects();
            if n100 > 0 {
                score.count300 = total_objects - n100 - miss;
                score.count100 = n100;
            } else {
                let target_total = (acc * 2.0 * total_objects as f32) as u32;
                score.count300 = target_total + miss - total_objects;
                score.count100 = total_objects - score.count300 - miss;
            }
            score.count_miss = miss;
            score.recalculate_grade(GameMode::TKO, Some(acc * 100.0));
        }
        GameMode::CTB => panic!("Can not simulate CTB scores"),
    }
}

pub fn unchoke_score(score: &mut Score, map: &Beatmap) {
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
            score.count300 += total_objects - passed_objects;
            let count_hits = total_objects - score.count_miss;
            let ratio = score.count300 as f32 / count_hits as f32;
            let new100s = (ratio * score.count_miss as f32) as u32;
            score.count100 += new100s;
            score.count300 += score.count_miss - new100s;
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
            score.grade = if score.enabled_mods.contains(&GameMod::Hidden) {
                Grade::XH
            } else {
                Grade::X
            };
        }
        _ => panic!("Can only unchoke STD and MNA scores, not {:?}", map.mode,),
    }
}
