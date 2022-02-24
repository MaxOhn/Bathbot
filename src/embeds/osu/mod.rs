mod avatar;
mod bws;
mod common;
mod compare;
mod country_snipe_list;
mod country_snipe_stats;
mod fix_score;
mod leaderboard;
mod map;
mod map_search;
mod match_costs;
mod match_live;
mod medal;
mod medal_stats;
mod medals_common;
mod medals_list;
mod medals_missing;
mod most_played;
mod most_played_common;
mod nochoke;
mod osekai_medal_count;
mod osekai_medal_rarity;
mod osustats_counts;
mod osustats_globals;
mod osustats_list;
mod pinned;
mod player_snipe_list;
mod player_snipe_stats;
mod pp_missing;
mod profile;
mod profile_compare;
mod rank;
mod rank_score;
mod ranking;
mod ranking_countries;
mod ratio;
mod recent;
mod recent_list;
mod scores;
mod simulate;
mod sniped;
mod sniped_difference;
mod top;
mod top_if;
mod top_single;
mod whatif;

use std::fmt::Write;

use rosu_pp::Mods;
use rosu_v2::prelude::{Beatmap, GameMode, GameMods};

use crate::util::{datetime::sec_to_minsec, numbers::round, BeatmapExt, ScoreExt};

pub use self::{
    avatar::AvatarEmbed,
    bws::BWSEmbed,
    common::CommonEmbed,
    compare::{CompareEmbed, NoScoresEmbed},
    country_snipe_list::CountrySnipeListEmbed,
    country_snipe_stats::CountrySnipeStatsEmbed,
    fix_score::FixScoreEmbed,
    leaderboard::LeaderboardEmbed,
    map::MapEmbed,
    map_search::MapSearchEmbed,
    match_costs::MatchCostEmbed,
    match_live::{MatchLiveEmbed, MatchLiveEmbeds},
    medal::MedalEmbed,
    medal_stats::MedalStatsEmbed,
    medals_common::{MedalsCommonEmbed, MedalsCommonUser},
    medals_list::MedalsListEmbed,
    medals_missing::MedalsMissingEmbed,
    most_played::MostPlayedEmbed,
    most_played_common::MostPlayedCommonEmbed,
    nochoke::NoChokeEmbed,
    osekai_medal_count::MedalCountEmbed,
    osekai_medal_rarity::MedalRarityEmbed,
    osustats_counts::OsuStatsCountsEmbed,
    osustats_globals::OsuStatsGlobalsEmbed,
    osustats_list::OsuStatsListEmbed,
    pinned::PinnedEmbed,
    player_snipe_list::PlayerSnipeListEmbed,
    player_snipe_stats::PlayerSnipeStatsEmbed,
    pp_missing::PPMissingEmbed,
    profile::ProfileEmbed,
    profile_compare::ProfileCompareEmbed,
    rank::RankEmbed,
    rank_score::RankRankedScoreEmbed,
    ranking::*,
    ranking_countries::RankingCountriesEmbed,
    ratio::RatioEmbed,
    recent::RecentEmbed,
    recent_list::RecentListEmbed,
    scores::ScoresEmbed,
    simulate::SimulateEmbed,
    sniped::SnipedEmbed,
    sniped_difference::SnipedDiffEmbed,
    top::{OrderAppendix, TopEmbed},
    top_if::TopIfEmbed,
    top_single::TopSingleEmbed,
    whatif::WhatIfEmbed,
};

pub fn get_stars(stars: f32) -> String {
    format!("{:.2}★", stars)
}

pub fn get_mods(mods: GameMods) -> String {
    if mods.is_empty() {
        String::new()
    } else {
        format!("+{mods}")
    }
}

pub fn get_combo(score: &dyn ScoreExt, map: &dyn BeatmapExt) -> String {
    let mut combo = String::from("**");
    let _ = write!(combo, "{}x**/", score.max_combo());

    match map.max_combo() {
        Some(amount) => write!(combo, "{amount}x").unwrap(),
        None => combo.push('-'),
    }

    combo
}

pub fn get_pp(actual: Option<f32>, max: Option<f32>) -> String {
    let mut result = String::with_capacity(17);
    result.push_str("**");

    if let Some(pp) = actual {
        let _ = write!(result, "{:.2}", pp);
    } else {
        result.push('-');
    }

    result.push_str("**/");

    if let Some(max) = max {
        let pp = actual.map(|pp| pp.max(max)).unwrap_or(max);
        let _ = write!(result, "{:.2}", pp);
    } else {
        result.push('-');
    }

    result.push_str("PP");

    result
}

pub fn get_keys(mods: GameMods, map: &Beatmap) -> String {
    if let Some(key_mod) = mods.has_key_mod() {
        format!("[{key_mod}]")
    } else {
        format!("[{}K]", map.cs as u32)
    }
}

pub fn calculate_od(od: f32, clock_rate: f32) -> f32 {
    let ms = difficulty_range(od, OD_MIN, OD_MID, OD_MAX) / clock_rate;

    (OD_MIN - ms) / (OD_MIN - OD_MID) * 5.0
}

const OD_MIN: f32 = 80.0;
const OD_MID: f32 = 50.0;
const OD_MAX: f32 = 20.0;

pub fn calculate_ar(ar: f32, clock_rate: f32) -> f32 {
    let ms = difficulty_range(ar, AR_MIN, AR_MID, AR_MAX) / clock_rate;

    if ms > AR_MID {
        (AR_MIN - ms) / (AR_MIN - AR_MID) * 5.0
    } else {
        (AR_MID - ms) / (AR_MID - AR_MAX) * 5.0 + 5.0
    }
}

const AR_MIN: f32 = 1800.0;
const AR_MID: f32 = 1200.0;
const AR_MAX: f32 = 450.0;

fn difficulty_range(difficulty: f32, min: f32, mid: f32, max: f32) -> f32 {
    if difficulty > 5.0 {
        mid + (max - mid) * (difficulty - 5.0) / 5.0
    } else if difficulty < 5.0 {
        mid - (mid - min) * (5.0 - difficulty) / 5.0
    } else {
        mid
    }
}

/// The stars argument must already be adjusted for mods
pub fn get_map_info(map: &Beatmap, mods: GameMods, stars: f32) -> String {
    let clock_rate = mods.bits().speed();

    let mut sec_total = map.seconds_total;
    let mut sec_drain = map.seconds_drain;
    let mut bpm = map.bpm;
    let mut cs = map.cs;
    let mut ar = map.ar;
    let mut od = map.od;
    let mut hp = map.hp;

    if mods.contains(GameMods::HardRock) {
        hp = (hp * 1.4).min(10.0);
        od = (od * 1.4).min(10.0);
        ar = (ar * 1.4).min(10.0);
        cs = (cs * 1.3).min(10.0);
    } else if mods.contains(GameMods::Easy) {
        hp *= 0.5;
        od *= 0.5;
        ar *= 0.5;
        cs *= 0.5;
    }

    if (clock_rate - 1.0).abs() > f64::EPSILON {
        let clock_rate = clock_rate as f32;

        bpm *= clock_rate;
        sec_total = (sec_total as f32 / clock_rate) as u32;
        sec_drain = (sec_drain as f32 / clock_rate) as u32;

        od = calculate_od(od, clock_rate);
        ar = calculate_ar(ar, clock_rate);
    }

    if map.mode == GameMode::MNA {
        ar = map.ar;
        od = map.od;
    }

    let mut map_info = String::with_capacity(128);

    let _ = write!(map_info, "Length: `{}` ", sec_to_minsec(sec_total));

    if sec_drain != sec_total {
        let _ = write!(map_info, "(`{}`) ", sec_to_minsec(sec_drain));
    }

    let _ = write!(
        map_info,
        "BPM: `{}` Objects: `{}`\n\
        CS: `{}` AR: `{}` OD: `{}` HP: `{}` Stars: `{}`",
        round(bpm),
        map.count_objects(),
        round(cs),
        round(ar),
        round(od),
        round(hp),
        round(stars)
    );

    map_info
}
