use crate::{
    custom_client::{OsuStatsScore, ScraperScore},
    util::{numbers::round, osu::grade_emote},
};

use rosu_pp::ScoreState;
use rosu_v2::prelude::{GameMode, GameMods, Grade, MatchScore, Score};
use std::fmt::Write;

pub trait ScoreExt: Send + Sync {
    // Required to implement
    fn count_miss(&self) -> u32;
    fn count_50(&self) -> u32;
    fn count_100(&self) -> u32;
    fn count_300(&self) -> u32;
    fn count_geki(&self) -> u32;
    fn count_katu(&self) -> u32;
    fn max_combo(&self) -> u32;
    fn mods(&self) -> GameMods;
    fn score(&self) -> u32;
    fn pp(&self) -> Option<f32>;
    fn acc(&self, mode: GameMode) -> f32;

    // Optional to implement
    fn grade(&self, mode: GameMode) -> Grade {
        match mode {
            GameMode::Osu => self.osu_grade(),
            GameMode::Mania => self.mania_grade(Some(self.acc(GameMode::Mania))),
            GameMode::Catch => self.ctb_grade(Some(self.acc(GameMode::Catch))),
            GameMode::Taiko => self.taiko_grade(),
        }
    }
    fn hits(&self, mode: u8) -> u32 {
        let mut amount = self.count_300() + self.count_100() + self.count_miss();

        if mode != 1 {
            // TKO
            amount += self.count_50();

            if mode != 0 {
                // STD
                amount += self.count_katu();

                // CTB
                amount += (mode != 2) as u32 * self.count_geki();
            }
        }

        amount
    }

    #[inline]
    fn is_fc(&self, mode: GameMode, max_combo: u32) -> bool {
        match mode {
            _ if self.count_miss() > 0 || self.grade(mode) == Grade::F => false,
            // Allow 1 missed sliderend per 500 combo
            GameMode::Osu => self.max_combo() >= (max_combo - (max_combo / 500).max(4)),
            GameMode::Taiko | GameMode::Mania => true,
            GameMode::Catch => self.max_combo() == max_combo,
        }
    }

    #[inline]
    fn state(&self) -> ScoreState {
        ScoreState {
            max_combo: self.max_combo() as usize,
            misses: self.count_miss() as usize,
            n300: self.count_300() as usize,
            n_katu: self.count_katu() as usize,
            n100: self.count_100() as usize,
            n50: self.count_50() as usize,
            score: self.score(),
        }
    }

    // Processing to strings
    #[inline]
    fn grade_emote(&self, mode: GameMode) -> &'static str {
        grade_emote(self.grade(mode))
    }
    fn hits_string(&self, mode: GameMode) -> String {
        let mut hits = String::from("{");
        if mode == GameMode::Mania {
            let _ = write!(hits, "{}/", self.count_geki());
        }
        let _ = write!(hits, "{}/", self.count_300());
        if mode == GameMode::Mania {
            let _ = write!(hits, "{}/", self.count_katu());
        }
        let _ = write!(hits, "{}/", self.count_100());
        if mode != GameMode::Taiko {
            let _ = write!(hits, "{}/", self.count_50());
        }
        let _ = write!(hits, "{}}}", self.count_miss());
        hits
    }

    // #########################
    // ## Auxiliary functions ##
    // #########################
    fn osu_grade(&self) -> Grade {
        let passed_objects = self.hits(GameMode::Osu as u8);
        let mods = self.mods();

        if self.count_300() == passed_objects {
            return if mods.contains(GameMods::Hidden) || mods.contains(GameMods::Flashlight) {
                Grade::XH
            } else {
                Grade::X
            };
        }

        let ratio300 = self.count_300() as f32 / passed_objects as f32;
        let ratio50 = self.count_50() as f32 / passed_objects as f32;

        if ratio300 > 0.9 && ratio50 < 0.01 && self.count_miss() == 0 {
            if mods.contains(GameMods::Hidden) || mods.contains(GameMods::Flashlight) {
                Grade::SH
            } else {
                Grade::S
            }
        } else if ratio300 > 0.9 || (ratio300 > 0.8 && self.count_miss() == 0) {
            Grade::A
        } else if ratio300 > 0.8 || (ratio300 > 0.7 && self.count_miss() == 0) {
            Grade::B
        } else if ratio300 > 0.6 {
            Grade::C
        } else {
            Grade::D
        }
    }

    fn mania_grade(&self, acc: Option<f32>) -> Grade {
        let passed_objects = self.hits(GameMode::Mania as u8);
        let mods = self.mods();

        if self.count_geki() == passed_objects {
            return if mods.contains(GameMods::Hidden) || mods.contains(GameMods::Flashlight) {
                Grade::XH
            } else {
                Grade::X
            };
        }

        let acc = acc.unwrap_or_else(|| self.acc(GameMode::Mania));

        if acc > 95.0 {
            if mods.contains(GameMods::Hidden) || mods.contains(GameMods::Flashlight) {
                Grade::SH
            } else {
                Grade::S
            }
        } else if acc > 90.0 {
            Grade::A
        } else if acc > 80.0 {
            Grade::B
        } else if acc > 70.0 {
            Grade::C
        } else {
            Grade::D
        }
    }

    fn taiko_grade(&self) -> Grade {
        let mods = self.mods();
        let passed_objects = self.hits(GameMode::Taiko as u8);
        let count_300 = self.count_300();

        if count_300 == passed_objects {
            return if mods.intersects(GameMods::Hidden | GameMods::Flashlight) {
                Grade::XH
            } else {
                Grade::X
            };
        }

        let ratio300 = count_300 as f32 / passed_objects as f32;
        let count_miss = self.count_miss();

        if ratio300 > 0.9 && count_miss == 0 {
            if mods.intersects(GameMods::Hidden | GameMods::Flashlight) {
                Grade::SH
            } else {
                Grade::S
            }
        } else if ratio300 > 0.9 || (ratio300 > 0.8 && count_miss == 0) {
            Grade::A
        } else if ratio300 > 0.8 || (ratio300 > 0.7 && count_miss == 0) {
            Grade::B
        } else if ratio300 > 0.6 {
            Grade::C
        } else {
            Grade::D
        }
    }

    fn ctb_grade(&self, acc: Option<f32>) -> Grade {
        let mods = self.mods();
        let acc = acc.unwrap_or_else(|| self.acc(GameMode::Catch));

        if (100.0 - acc).abs() <= std::f32::EPSILON {
            if mods.contains(GameMods::Hidden) || mods.contains(GameMods::Flashlight) {
                Grade::XH
            } else {
                Grade::X
            }
        } else if acc > 98.0 {
            if mods.contains(GameMods::Hidden) || mods.contains(GameMods::Flashlight) {
                Grade::SH
            } else {
                Grade::S
            }
        } else if acc > 94.0 {
            Grade::A
        } else if acc > 90.0 {
            Grade::B
        } else if acc > 85.0 {
            Grade::C
        } else {
            Grade::D
        }
    }
}

// #####################
// ## Implementations ##
// #####################

impl ScoreExt for Score {
    #[inline]
    fn count_miss(&self) -> u32 {
        self.statistics.count_miss
    }
    #[inline]
    fn count_50(&self) -> u32 {
        self.statistics.count_50
    }
    #[inline]
    fn count_100(&self) -> u32 {
        self.statistics.count_100
    }
    #[inline]
    fn count_300(&self) -> u32 {
        self.statistics.count_300
    }
    #[inline]
    fn count_geki(&self) -> u32 {
        self.statistics.count_geki
    }
    #[inline]
    fn count_katu(&self) -> u32 {
        self.statistics.count_katu
    }
    #[inline]
    fn max_combo(&self) -> u32 {
        self.max_combo
    }
    #[inline]
    fn mods(&self) -> GameMods {
        self.mods
    }
    #[inline]
    fn grade(&self, _mode: GameMode) -> Grade {
        self.grade
    }
    #[inline]
    fn score(&self) -> u32 {
        self.score
    }
    #[inline]
    fn pp(&self) -> Option<f32> {
        self.pp
    }
    #[inline]
    fn acc(&self, _: GameMode) -> f32 {
        round(self.accuracy)
    }
}

impl ScoreExt for OsuStatsScore {
    #[inline]
    fn count_miss(&self) -> u32 {
        self.count_miss
    }
    #[inline]
    fn count_50(&self) -> u32 {
        self.count50
    }
    #[inline]
    fn count_100(&self) -> u32 {
        self.count100
    }
    #[inline]
    fn count_300(&self) -> u32 {
        self.count300
    }
    #[inline]
    fn count_geki(&self) -> u32 {
        self.count_geki
    }
    #[inline]
    fn count_katu(&self) -> u32 {
        self.count_katu
    }
    #[inline]
    fn max_combo(&self) -> u32 {
        self.max_combo
    }
    #[inline]
    fn mods(&self) -> GameMods {
        self.enabled_mods
    }
    fn hits(&self, _mode: u8) -> u32 {
        let mut amount = self.count300 + self.count100 + self.count_miss;
        let mode = self.map.mode;

        if mode != GameMode::Taiko {
            amount += self.count50;

            if mode != GameMode::Osu {
                amount += self.count_katu;
                amount += (mode != GameMode::Catch) as u32 * self.count_geki;
            }
        }

        amount
    }
    #[inline]
    fn grade(&self, _: GameMode) -> Grade {
        self.grade
    }
    #[inline]
    fn score(&self) -> u32 {
        self.score
    }
    #[inline]
    fn pp(&self) -> Option<f32> {
        self.pp
    }
    #[inline]
    fn acc(&self, _: GameMode) -> f32 {
        self.accuracy
    }
}

impl ScoreExt for ScraperScore {
    #[inline]
    fn count_miss(&self) -> u32 {
        self.count_miss
    }
    #[inline]
    fn count_50(&self) -> u32 {
        self.count50
    }
    #[inline]
    fn count_100(&self) -> u32 {
        self.count100
    }
    #[inline]
    fn count_300(&self) -> u32 {
        self.count300
    }
    #[inline]
    fn count_geki(&self) -> u32 {
        self.count_geki
    }
    #[inline]
    fn count_katu(&self) -> u32 {
        self.count_katu
    }
    #[inline]
    fn max_combo(&self) -> u32 {
        self.max_combo
    }
    #[inline]
    fn mods(&self) -> GameMods {
        self.mods
    }
    #[inline]
    fn grade(&self, _: GameMode) -> Grade {
        self.grade
    }
    #[inline]
    fn score(&self) -> u32 {
        self.score
    }
    #[inline]
    fn pp(&self) -> Option<f32> {
        self.pp
    }
    #[inline]
    fn acc(&self, _: GameMode) -> f32 {
        self.accuracy
    }
}

impl ScoreExt for MatchScore {
    #[inline]
    fn count_miss(&self) -> u32 {
        self.statistics.count_miss
    }

    #[inline]
    fn count_50(&self) -> u32 {
        self.statistics.count_50
    }

    #[inline]
    fn count_100(&self) -> u32 {
        self.statistics.count_100
    }

    #[inline]
    fn count_300(&self) -> u32 {
        self.statistics.count_300
    }

    #[inline]
    fn count_geki(&self) -> u32 {
        self.statistics.count_geki
    }

    #[inline]
    fn count_katu(&self) -> u32 {
        self.statistics.count_katu
    }

    #[inline]
    fn max_combo(&self) -> u32 {
        self.max_combo
    }

    #[inline]
    fn mods(&self) -> GameMods {
        self.mods
    }

    #[inline]
    fn score(&self) -> u32 {
        self.score
    }

    #[inline]
    fn pp(&self) -> Option<f32> {
        None
    }

    #[inline]
    fn acc(&self, _: GameMode) -> f32 {
        self.accuracy
    }
}
