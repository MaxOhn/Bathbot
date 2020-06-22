mod common;
mod leaderboard;
mod match_costs;
mod most_played;
mod most_played_common;
mod nochoke;
mod osustats_globals;
mod pp_missing;
mod profile;
mod rank;
mod ratio;
mod recent;
mod scores;
mod simulate;
mod top;
mod whatif;

pub use common::CommonEmbed;
pub use leaderboard::LeaderboardEmbed;
pub use match_costs::MatchCostEmbed;
pub use most_played::MostPlayedEmbed;
pub use most_played_common::MostPlayedCommonEmbed;
pub use nochoke::NoChokeEmbed;
pub use osustats_globals::OsuStatsGlobalsEmbed;
pub use pp_missing::PPMissingEmbed;
pub use profile::ProfileEmbed;
pub use rank::RankEmbed;
pub use ratio::RatioEmbed;
pub use recent::RecentEmbed;
pub use scores::ScoresEmbed;
pub use simulate::SimulateEmbed;
pub use top::TopEmbed;
pub use whatif::WhatIfEmbed;

use crate::{
    embeds::Author,
    util::{datetime::sec_to_minsec, globals::HOMEPAGE, numbers, osu::grade_emote},
};

use rosu::models::{Beatmap, GameMode, GameMods, Grade, Score, User};
use serenity::cache::Cache;
use std::fmt::Write;

pub fn get_user_author(user: &User) -> Author {
    let text = format!(
        "{name}: {pp}pp (#{global} {country}{national})",
        name = user.username,
        pp = numbers::round_and_comma(user.pp_raw),
        global = numbers::with_comma_u64(user.pp_rank as u64),
        country = user.country,
        national = user.pp_country_rank
    );
    Author::new(text)
        .url(format!("{}u/{}", HOMEPAGE, user.user_id))
        .icon_url(format!("{}/images/flags/{}.png", HOMEPAGE, user.country))
}

pub fn get_stars(stars: f32) -> String {
    format!("{}★", numbers::round(stars))
}

pub fn get_mods(mods: GameMods) -> String {
    if mods.is_empty() {
        String::new()
    } else {
        let mut res = String::new();
        let _ = write!(res, "+{}", mods);
        res
    }
}

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
    format!("{}%", numbers::round(score.accuracy(mode)))
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

pub fn get_pp(actual: Option<f32>, max: Option<f32>) -> String {
    let actual = actual.map_or_else(|| '-'.to_string(), |pp| numbers::round(pp).to_string());
    let max = max.map_or_else(|| '-'.to_string(), |pp| numbers::round(pp).to_string());
    format!("**{}**/{}PP", actual, max)
}

pub fn get_keys(mods: GameMods, map: &Beatmap) -> String {
    if let Some(key_mod) = mods.has_key_mod() {
        format!("[{}]", key_mod)
    } else {
        format!("[{}K]", map.diff_cs as u32)
    }
}

pub async fn get_grade_completion_mods(score: &Score, map: &Beatmap, cache: &Cache) -> String {
    let mut res_string = grade_emote(score.grade, cache).await.to_string();
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
        numbers::round(map.bpm).to_string(),
        map.count_objects(),
        numbers::round(map.diff_cs).to_string(),
        numbers::round(map.diff_ar).to_string(),
        numbers::round(map.diff_od).to_string(),
        numbers::round(map.diff_hp).to_string(),
        numbers::round(map.stars)
    )
}
