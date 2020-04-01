use crate::util::{datetime::sec_to_minsec, numbers::round, osu, pp::PPProvider};

use rosu::models::{Beatmap, GameMode, GameMods, Score};
use serenity::cache::CacheRwLock;

pub fn get_hits(score: &Score, mode: GameMode) -> String {
    let mut hits = String::from("{");
    if mode == GameMode::MNA {
        hits.push_str(&score.count_geki.to_string());
        hits.push('/');
    }
    hits.push_str(&score.count300.to_string());
    hits.push('/');
    if mode == GameMode::MNA {
        hits.push_str(&score.count_katu.to_string());
        hits.push('/');
    }
    hits.push_str(&score.count100.to_string());
    hits.push('/');
    if mode != GameMode::TKO {
        hits.push_str(&score.count50.to_string());
        hits.push('/');
    }
    hits.push_str(&score.count_miss.to_string());
    hits.push('}');
    hits
}

pub fn get_acc(score: &Score, mode: GameMode) -> String {
    format!("{}%", round(score.accuracy(mode)))
}

pub fn get_combo(score: &Score, map: &Beatmap) -> String {
    let mut combo = String::from("**");
    combo.push_str(&score.max_combo.to_string());
    combo.push_str("x**/");
    match map.max_combo {
        Some(amount) => {
            combo.push_str(&amount.to_string());
            combo.push('x');
        }
        None => combo.push('-'),
    }
    combo
}

pub fn get_pp(score: &Score, pp_provider: &PPProvider, mode: GameMode) -> String {
    let (actual, max) = if mode == GameMode::CTB {
        (score.pp, None)
    } else {
        let actual = score.pp.or_else(|| Some(pp_provider.pp()));
        let max = pp_provider.max_pp();
        (actual, Some(max))
    };
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
        res.push('+');
        res.push_str(&mods.to_string());
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
    let passed = score.total_hits(map.mode);
    let total = map.count_objects();
    if passed < total {
        res_string.push_str(" (");
        res_string.push_str(&(100 * passed / total).to_string());
        res_string.push_str("%)");
    }
    if !score.enabled_mods.is_empty() {
        res_string.push_str(" +");
        res_string.push_str(&score.enabled_mods.to_string());
    }
    res_string
}

pub fn get_map_info(map: &Beatmap) -> String {
    #![allow(clippy::float_cmp)]
    format!(
        "Length: `{}` (`{}`) BPM: `{}` Objects: `{}`\nCS: `{}` AR: `{}` OD: `{}` HP: `{}` Stars: `{}`",
        sec_to_minsec(map.seconds_total),
        sec_to_minsec(map.seconds_drain),
        if map.bpm == map.bpm.round() {
            (map.bpm as u32).to_string()
        } else {
            round(map.bpm).to_string()
        },
        map.count_objects(),
        if map.diff_cs == map.diff_cs.round() {
            (map.diff_cs as u32).to_string()
        } else {
            round(map.diff_cs).to_string()
        },
        if map.diff_ar == map.diff_ar.round() {
            (map.diff_ar as u32).to_string()
        } else {
            round(map.diff_ar).to_string()
        },
        if map.diff_od == map.diff_od.round() {
            (map.diff_od as u32).to_string()
        } else {
            round(map.diff_od).to_string()
        },
        if map.diff_hp == map.diff_hp.round() {
            (map.diff_hp as u32).to_string()
        } else {
            round(map.diff_hp).to_string()
        },
        round(map.stars)
    )
}
