mod common;
mod leaderboard;
mod map;
mod match_costs;
mod most_played;
mod most_played_common;
mod nochoke;
mod osustats_counts;
mod osustats_globals;
mod player_snipe_stats;
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
pub use map::MapEmbed;
pub use match_costs::MatchCostEmbed;
pub use most_played::MostPlayedEmbed;
pub use most_played_common::MostPlayedCommonEmbed;
pub use nochoke::NoChokeEmbed;
pub use osustats_counts::OsuStatsCountsEmbed;
pub use osustats_globals::OsuStatsGlobalsEmbed;
pub use player_snipe_stats::PlayerSnipeStatsEmbed;
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
    util::{
        constants::OSU_BASE,
        datetime::sec_to_minsec,
        numbers::{round, round_and_comma, with_comma_int},
        BeatmapExt, ScoreExt,
    },
};

use rosu::models::{Beatmap, GameMods, User};
use std::{borrow::Cow, fmt::Write};

pub fn get_user_author(user: &User) -> Author {
    let text = format!(
        "{name}: {pp}pp (#{global} {country}{national})",
        name = user.username,
        pp = round_and_comma(user.pp_raw),
        global = with_comma_int(user.pp_rank),
        country = user.country,
        national = user.pp_country_rank
    );
    Author::new(text)
        .url(format!("{}u/{}", OSU_BASE, user.user_id))
        .icon_url(format!("{}/images/flags/{}.png", OSU_BASE, user.country))
}

pub fn get_stars(stars: f32) -> String {
    format!("{}★", round(stars))
}

pub fn get_mods(mods: GameMods) -> String {
    if mods.is_empty() {
        String::new()
    } else {
        format!("+{}", mods)
    }
}

pub fn get_combo(score: impl ScoreExt, map: impl BeatmapExt) -> String {
    let mut combo = String::from("**");
    let _ = write!(combo, "{}x**/", score.max_combo());
    match map.max_combo() {
        Some(amount) => {
            let _ = write!(combo, "{}x", amount);
        }
        None => combo.push('-'),
    }
    combo
}

pub fn get_pp(actual: Option<f32>, max: Option<f32>) -> String {
    let actual = actual.map_or_else(
        || Cow::Borrowed("-"),
        |pp| Cow::Owned(round(pp).to_string()),
    );
    let max = max.map_or_else(
        || Cow::Borrowed("-"),
        |pp| Cow::Owned(round(pp).to_string()),
    );
    format!("**{}**/{}PP", actual, max)
}

pub fn get_keys(mods: GameMods, map: &Beatmap) -> String {
    if let Some(key_mod) = mods.has_key_mod() {
        format!("[{}]", key_mod)
    } else {
        format!("[{}K]", map.diff_cs as u32)
    }
}

pub fn get_map_info(map: &Beatmap) -> String {
    format!(
        "Length: `{}` (`{}`) BPM: `{}` Objects: `{}`\n\
        CS: `{}` AR: `{}` OD: `{}` HP: `{}` Stars: `{}`",
        sec_to_minsec(map.seconds_total),
        sec_to_minsec(map.seconds_drain),
        round(map.bpm),
        map.count_objects(),
        round(map.diff_cs),
        round(map.diff_ar),
        round(map.diff_od),
        round(map.diff_hp),
        round(map.stars)
    )
}
