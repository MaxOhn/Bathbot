use crate::util::{datetime::sec_to_minsec, numbers::round, osu, pp::PPProvider};

use rosu::models::{Beatmap, GameMode, GameMods, Grade, Score};
use serenity::cache::CacheRwLock;
use std::fmt::Write;

pub fn get_hits(score: &Score, mode: GameMode) -> String {
    let mut hits = String::from("{");
    if mode == GameMode::MNA {
        let _ = write!(hits, "{}/", score.count_geki);
    }
    let _ = write!(hits, "{}/", score.count300);
    if mode == GameMode::MNA {
        let _ = write!(hits, "{}/", score.count_katu);
    }
    let _ = write!(hits, "{}/", score.count100);
    if mode != GameMode::TKO {
        let _ = write!(hits, "{}/", score.count50);
    }
    let _ = write!(hits, "{}}}", score.count_miss);
    hits
}

pub fn get_acc(score: &Score, mode: GameMode) -> String {
    format!("{}%", round(score.accuracy(mode)))
}

pub fn get_combo(score: &Score, map: &Beatmap) -> String {
    let mut combo = String::from("**");
    let _ = write!(combo, "{}x**/", score.max_combo);
    match map.max_combo {
        Some(amount) => {
            let _ = write!(combo, "{}x", amount);
        }
        None => combo.push('-'),
    }
    combo
}

pub fn get_pp(score: &Score, pp_provider: &PPProvider) -> String {
    let actual = score.pp.or_else(|| Some(pp_provider.pp()));
    let max = Some(pp_provider.max_pp());
    _get_pp(actual, max)
}

pub fn _get_pp(actual: Option<f32>, max: Option<f32>) -> String {
    let actual = actual.map_or_else(|| String::from("-"), |pp| round(pp).to_string());
    let max = max.map_or_else(|| String::from("-"), |pp| round(pp).to_string());
    format!("**{}**/{}PP", actual, max)
}

pub fn get_mods(mods: &GameMods) -> String {
    if mods.is_empty() {
        String::new()
    } else {
        let mut res = String::new();
        let _ = write!(res, "+{}", mods);
        res
    }
}

pub fn get_keys(mods: &GameMods, map: &Beatmap) -> String {
    for m in mods.iter() {
        if m.is_key_mod() {
            return format!("[{}]", m);
        }
    }
    format!("[{}K]", map.diff_cs as u32)
}

pub fn get_stars(stars: f32) -> String {
    format!("{}★", round(stars))
}

pub async fn get_grade_completion_mods(score: &Score, map: &Beatmap, cache: CacheRwLock) -> String {
    let mut res_string = osu::grade_emote(score.grade, cache).await.to_string();
    if score.grade == Grade::F && map.mode != GameMode::CTB {
        let passed = score.total_hits(map.mode) - score.count50;
        let total = map.count_objects();
        let _ = write!(res_string, " ({}%)", 100 * passed / total);
    }
    if !score.enabled_mods.is_empty() {
        let _ = write!(res_string, " +{}", score.enabled_mods);
    }
    res_string
}

pub fn get_map_info(map: &Beatmap) -> String {
    format!(
        "Length: `{}` (`{}`) BPM: `{}` Objects: `{}`\n\
        CS: `{}` AR: `{}` OD: `{}` HP: `{}` Stars: `{}`",
        sec_to_minsec(map.seconds_total),
        sec_to_minsec(map.seconds_drain),
        round(map.bpm).to_string(),
        map.count_objects(),
        round(map.diff_cs).to_string(),
        round(map.diff_ar).to_string(),
        round(map.diff_od).to_string(),
        round(map.diff_hp).to_string(),
        round(map.stars)
    )
}
