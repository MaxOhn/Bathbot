mod attributes;
mod badge;
mod bws;
mod claim_name;
mod common;
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
mod serverscores;
mod simulate;
mod sniped;
mod sniped_difference;
mod top;
mod top_if;
mod top_single;
mod whatif;

use std::fmt::{Display, Formatter, Result as FmtResult};

use rosu_v2::prelude::{GameModIntermode, GameMode, GameMods, ScoreStatistics};

#[cfg(feature = "matchlive")]
pub use self::match_live::*;
pub use self::{
    attributes::*, badge::*, bws::*, claim_name::*, common::*, country_snipe_list::*,
    country_snipe_stats::*, fix_score::*, leaderboard::*, map::*, map_search::*, match_compare::*,
    match_costs::*, medal::*, medal_stats::*, medals_common::*, medals_list::*, medals_missing::*,
    most_played::*, most_played_common::*, nochoke::*, osekai_medal_count::*,
    osekai_medal_rarity::*, osustats_counts::*, osustats_globals::*, osustats_list::*,
    osutracker_countrytop::*, osutracker_mappers::*, osutracker_maps::*, osutracker_mapsets::*,
    osutracker_mods::*, player_snipe_list::*, player_snipe_stats::*, pp_missing::*, profile::*,
    profile_compare::*, rank::*, rank_score::*, ranking::*, ranking_countries::*, ratio::*,
    recent::*, recent_list::*, scores::*, serverscores::*, simulate::*, sniped::*,
    sniped_difference::*, top::*, top_if::*, top_single::*, whatif::*,
};
use crate::manager::OsuMap;

pub struct ModsFormatter<'m> {
    mods: &'m GameMods,
}

impl<'m> ModsFormatter<'m> {
    pub fn new(mods: &'m GameMods) -> Self {
        Self { mods }
    }
}

impl Display for ModsFormatter<'_> {
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

pub struct KeyFormatter<'m> {
    mods: &'m GameMods,
    cs: u32,
}

impl<'m> KeyFormatter<'m> {
    pub fn new(mods: &'m GameMods, map: &OsuMap) -> Self {
        Self {
            mods,
            cs: map.cs() as u32,
        }
    }
}

impl Display for KeyFormatter<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let key_mod = [
            GameModIntermode::OneKey,
            GameModIntermode::TwoKeys,
            GameModIntermode::ThreeKeys,
            GameModIntermode::FourKeys,
            GameModIntermode::FiveKeys,
            GameModIntermode::SixKeys,
            GameModIntermode::SevenKeys,
            GameModIntermode::EightKeys,
            GameModIntermode::NineKeys,
            GameModIntermode::TenKeys,
        ]
        .into_iter()
        .find(|gamemod| self.mods.contains_intermode(gamemod));

        match key_mod {
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
