mod attributes;
mod bws;
mod claim_name;
mod country_snipe_stats;
mod fix_score;
mod match_costs;
mod match_live;
mod medal_stats;
mod osustats_counts;
mod player_snipe_stats;
mod pp_missing;
mod profile_compare;
mod ratio;
mod sniped;
mod whatif;

use std::fmt::{Display, Formatter, Result as FmtResult};

use rosu_v2::prelude::{GameModIntermode, GameMode, GameMods, LegacyScoreStatistics};

#[cfg(feature = "matchlive")]
pub use self::match_live::*;
pub use self::{
    attributes::*, bws::*, claim_name::*, country_snipe_stats::*, fix_score::*, match_costs::*,
    medal_stats::*, osustats_counts::*, player_snipe_stats::*, pp_missing::*, profile_compare::*,
    ratio::*, sniped::*, whatif::*,
};

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
    pub fn new(mods: &'m GameMods, cs: f32) -> Self {
        Self {
            mods,
            cs: cs as u32,
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
    stats: LegacyScoreStatistics,
}

impl HitResultFormatter {
    pub fn new(mode: GameMode, stats: LegacyScoreStatistics) -> Self {
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
