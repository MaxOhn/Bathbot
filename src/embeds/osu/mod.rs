mod attributes;
mod badge;
mod bws;
mod card;
mod claim_name;
mod common;
mod compare;
mod country_snipe_list;
mod country_snipe_stats;
mod fix_score;
mod graph;
mod leaderboard;
mod map;
mod map_search;
mod match_compare;
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
mod osutracker_countrytop;
mod osutracker_mappers;
mod osutracker_maps;
mod osutracker_mapsets;
mod osutracker_mods;
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

use rosu_pp::beatmap::BeatmapAttributesBuilder;
use rosu_v2::prelude::{Beatmap, GameMode, GameMods};

use crate::util::{datetime::sec_to_minsec, numbers::round, BeatmapExt, ScoreExt};

pub use self::{
    attributes::*, badge::*, bws::*, card::*, claim_name::*, common::*, compare::*,
    country_snipe_list::*, country_snipe_stats::*, fix_score::*, graph::*, leaderboard::*, map::*,
    map_search::*, match_compare::*, match_costs::*, medal::*, medal_stats::*, medals_common::*,
    medals_list::*, medals_missing::*, most_played::*, most_played_common::*, nochoke::*,
    osekai_medal_count::*, osekai_medal_rarity::*, osustats_counts::*, osustats_globals::*,
    osustats_list::*, osutracker_countrytop::*, osutracker_mappers::*, osutracker_maps::*,
    osutracker_mapsets::*, osutracker_mods::*, player_snipe_list::*, player_snipe_stats::*,
    pp_missing::*, profile::*, profile_compare::*, rank::*, rank_score::*, ranking::*,
    ranking_countries::*, ratio::*, recent::*, recent_list::*, scores::*, simulate::*, sniped::*,
    sniped_difference::*, top::*, top_if::*, top_single::*, whatif::*,
};

#[cfg(feature = "matchlive")]
pub use self::match_live::*;

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

/// The stars argument must already be adjusted for mods
pub fn get_map_info(map: &Beatmap, mods: GameMods, stars: f32) -> String {
    let mode = match map.mode {
        GameMode::Osu => rosu_pp::GameMode::Osu,
        GameMode::Taiko => rosu_pp::GameMode::Taiko,
        GameMode::Catch => rosu_pp::GameMode::Catch,
        GameMode::Mania => rosu_pp::GameMode::Mania,
    };

    let attrs = BeatmapAttributesBuilder::default()
        .mode(mode)
        .ar(map.ar)
        .cs(map.cs)
        .hp(map.hp)
        .od(map.od)
        .mods(mods.bits())
        .converted(map.convert)
        .build();

    let clock_rate = attrs.clock_rate;
    let mut sec_total = map.seconds_total;
    let mut sec_drain = map.seconds_drain;
    let mut bpm = map.bpm;

    if (clock_rate - 1.0).abs() > f64::EPSILON {
        let clock_rate = clock_rate as f32;

        bpm *= clock_rate;
        sec_total = (sec_total as f32 / clock_rate) as u32;
        sec_drain = (sec_drain as f32 / clock_rate) as u32;
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
        round(attrs.cs as f32),
        round(attrs.ar as f32),
        round(attrs.od as f32),
        round(attrs.hp as f32),
        round(stars),
    );

    map_info
}
