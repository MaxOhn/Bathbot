mod attributes;
mod badge;
mod bws;
mod claim_name;
mod common;
mod compare;
mod country_snipe_list;
mod country_snipe_stats;
mod fix_score;
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

use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_util::{datetime::SecToMinSec, numbers::round};
use rosu_pp::beatmap::BeatmapAttributesBuilder;
use rosu_v2::prelude::{GameMode, GameMods, ScoreStatistics};

use crate::manager::OsuMap;

pub use self::{
    attributes::*, badge::*, bws::*, claim_name::*, common::*, compare::*, country_snipe_list::*,
    country_snipe_stats::*, fix_score::*, leaderboard::*, map::*, map_search::*, match_compare::*,
    match_costs::*, medal::*, medal_stats::*, medals_common::*, medals_list::*, medals_missing::*,
    most_played::*, most_played_common::*, nochoke::*, osekai_medal_count::*,
    osekai_medal_rarity::*, osustats_counts::*, osustats_globals::*, osustats_list::*,
    osutracker_countrytop::*, osutracker_mappers::*, osutracker_maps::*, osutracker_mapsets::*,
    osutracker_mods::*, player_snipe_list::*, player_snipe_stats::*, pp_missing::*, profile::*,
    profile_compare::*, rank::*, rank_score::*, ranking::*, ranking_countries::*, ratio::*,
    recent::*, recent_list::*, scores::*, simulate::*, sniped::*, sniped_difference::*, top::*,
    top_if::*, top_single::*, whatif::*,
};

#[cfg(feature = "matchlive")]
pub use self::match_live::*;

pub struct ModsFormatter {
    mods: GameMods,
}

impl ModsFormatter {
    pub fn new(mods: GameMods) -> Self {
        Self { mods }
    }
}

impl Display for ModsFormatter {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.mods.is_empty() {
            Ok(())
        } else {
            write!(f, "+{}", self.mods)
        }
    }
}

pub struct ComboFormatter {
    score: u32,
    max: Option<u32>,
}

impl ComboFormatter {
    pub fn new(score: u32, max: Option<u32>) -> Self {
        Self { score, max }
    }
}

impl Display for ComboFormatter {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "**{}x**/", self.score)?;

        match self.max {
            Some(combo) => write!(f, "{combo}x"),
            None => f.write_str("-"),
        }
    }
}

pub struct PpFormatter {
    actual: Option<f32>,
    max: Option<f32>,
}

impl PpFormatter {
    pub fn new(actual: Option<f32>, max: Option<f32>) -> Self {
        Self { actual, max }
    }
}

impl Display for PpFormatter {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match (self.actual, self.max) {
            (Some(actual), Some(max)) => {
                write!(f, "**{actual:.2}**/{max:.2}", max = max.max(actual))?
            }
            (Some(actual), None) => write!(f, "**{actual:.2}**/-")?,
            (None, Some(max)) => write!(f, "-/{max:.2}")?,
            (None, None) => f.write_str("-/-")?,
        }

        f.write_str("PP")
    }
}

pub struct KeyFormatter {
    mods: GameMods,
    cs: u32,
}

impl KeyFormatter {
    pub fn new(mods: GameMods, map: &OsuMap) -> Self {
        Self {
            mods,
            cs: map.cs() as u32,
        }
    }
}

impl Display for KeyFormatter {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.mods.has_key_mod() {
            Some(key_mod) => write!(f, "[{key_mod}]"),
            None => write!(f, "[{}K]", self.cs),
        }
    }
}

#[derive(Clone)]
pub struct HitResultFormatter {
    mode: GameMode,
    stats: ScoreStatistics,
}

impl HitResultFormatter {
    pub fn new(mode: GameMode, stats: ScoreStatistics) -> Self {
        Self { mode, stats }
    }
}

impl Display for HitResultFormatter {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("{")?;

        if self.mode == GameMode::Mania {
            write!(f, "{}/", self.stats.count_geki)?;
        }

        write!(f, "{}/", self.stats.count_300)?;

        if self.mode == GameMode::Mania {
            write!(f, "{}/", self.stats.count_katu)?;
        }

        write!(f, "{}/", self.stats.count_100)?;

        if self.mode != GameMode::Taiko {
            write!(f, "{}/", self.stats.count_50)?;
        }

        write!(f, "{}}}", self.stats.count_miss)
    }
}

/// The stars argument must already be adjusted for mods
pub fn get_map_info(map: &OsuMap, mods: GameMods, stars: f32) -> String {
    let mode = match map.mode() {
        GameMode::Osu => rosu_pp::GameMode::Osu,
        GameMode::Taiko => rosu_pp::GameMode::Taiko,
        GameMode::Catch => rosu_pp::GameMode::Catch,
        GameMode::Mania => rosu_pp::GameMode::Mania,
    };

    let attrs = BeatmapAttributesBuilder::new(&map.pp_map)
        .mode(mode)
        .mods(mods.bits())
        .build();

    let clock_rate = attrs.clock_rate;
    let mut sec_drain = map.seconds_drain();
    let mut bpm = map.bpm();

    if (clock_rate - 1.0).abs() > f64::EPSILON {
        let clock_rate = clock_rate as f32;

        bpm *= clock_rate;
        sec_drain = (sec_drain as f32 / clock_rate) as u32;
    }

    let mut map_info = String::with_capacity(128);

    let _ = write!(map_info, "Length: `{}` ", SecToMinSec::new(sec_drain));

    let _ = write!(
        map_info,
        "BPM: `{}` Objects: `{}`\n\
        CS: `{}` AR: `{}` OD: `{}` HP: `{}` Stars: `{}`",
        round(bpm),
        map.n_objects(),
        round(attrs.cs as f32),
        round(attrs.ar as f32),
        round(attrs.od as f32),
        round(attrs.hp as f32),
        round(stars),
    );

    map_info
}
