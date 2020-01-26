use crate::util::{
    datetime::sec_to_minsec,
    globals::*,
    numbers::{round, round_and_comma},
    osu::get_grade_emote,
};
use rosu::models::{Beatmap, GameMod, GameMods, GameMode, Grade, Score};
use serenity::{
    cache::CacheRwLock,
    model::{
        guild::Emoji,
        id::{EmojiId, GuildId},
    },
};

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

pub fn get_acc(score: &Score, mode: GameMode, map: &Beatmap) -> String {
    let objects = match score.grade {
        Grade::F => Some(map.count_objects()),
        _ => None,
    };
    format!("{}%", round(score.get_accuracy(mode)))
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

pub fn get_pp(actual: f32, max: f32) -> String {
    format!("**{}**/{}PP", round(actual), round(max))
}

pub fn get_mods(mods: &[GameMod]) -> String {
    if mods.is_empty() {
        String::new()
    } else {
        let mut res = String::new();
        res.push('+');
        for m in mods {
            res.push_str(&m.acronym());
        }
        res
    }
}

pub fn get_keys(mods: &[GameMod], map: &Beatmap) -> String {
    for m in mods {
        if m.is_key_mod() {
            return format!("[{}]", m.acronym());
        }
    }
    format!("[{}K]", map.diff_cs as u32)
}

pub fn get_stars(_mods: &[GameMod], map: &Beatmap) -> String {
    format!("{}★", round(map.stars))
}

pub fn get_grade_completion_mods(score: &Score, mode: GameMode, mods: &impl GameMods, map: &Beatmap, cache: CacheRwLock) -> String {
    let mut res_string = get_grade_emote(score.grade, cache).to_string();
    let passed = score.get_amount_hits(mode);
    let total = map.count_objects();
    if passed < total {

    }
    if !score.enabled_mods.is_empty() {
        res_string.push_str(" +");
        res_string.push_str(&mods.to_string());
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
