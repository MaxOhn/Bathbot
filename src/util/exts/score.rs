use crate::{
    core::CONFIG,
    custom_client::{OsuStatsScore, ScraperScore},
    util::osu::grade_emote,
};

use rosu::models::{GameMode, GameMods, Grade, Score};
use std::{borrow::Cow, fmt::Write};

pub trait ScoreExt: Sized {
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
            GameMode::STD => self.osu_grade(),
            GameMode::MNA => self.mania_grade(None),
            GameMode::CTB => self.ctb_grade(None),
            GameMode::TKO => self.taiko_grade(None),
        }
    }
    fn hits(&self, mode: GameMode) -> u32 {
        let mut amount = self.count_300() + self.count_100() + self.count_miss();
        if mode != GameMode::TKO {
            amount += self.count_50();
            if mode != GameMode::STD {
                amount += self.count_katu();
                if mode != GameMode::CTB {
                    amount += self.count_geki();
                }
            }
        }
        amount
    }

    // Processing to strings
    fn grade_emote(&self, mode: GameMode) -> String {
        grade_emote(self.grade(mode))
    }
    fn grade_completion_mods(&self, mode: GameMode) -> Cow<str> {
        let grade = CONFIG.get().unwrap().grade(self.grade(mode));
        let mods = self.mods();
        if mods.is_empty() {
            Cow::Borrowed(grade)
        } else {
            Cow::Owned(format!("{} +{}", grade, mods))
        }
    }
    fn hits_string(&self, mode: GameMode) -> String {
        let mut hits = String::from("{");
        if mode == GameMode::MNA {
            let _ = write!(hits, "{}/", self.count_geki());
        }
        let _ = write!(hits, "{}/", self.count_300());
        if mode == GameMode::MNA {
            let _ = write!(hits, "{}/", self.count_katu());
        }
        let _ = write!(hits, "{}/", self.count_100());
        if mode != GameMode::TKO {
            let _ = write!(hits, "{}/", self.count_50());
        }
        let _ = write!(hits, "{}}}", self.count_miss());
        hits
    }
    fn acc_string(&self, mode: GameMode) -> String {
        format!("{}%", self.acc(mode))
    }

    // #########################
    // ## Auxiliary functions ##
    // #########################
    fn osu_grade(&self) -> Grade {
        let passed_objects = self.hits(GameMode::STD);
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
        let passed_objects = self.hits(GameMode::MNA);
        let mods = self.mods();
        if self.count_geki() == passed_objects {
            return if mods.contains(GameMods::Hidden) || mods.contains(GameMods::Flashlight) {
                Grade::XH
            } else {
                Grade::X
            };
        }
        let acc = acc.unwrap_or_else(|| self.acc(GameMode::MNA));
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

    fn taiko_grade(&self, acc: Option<f32>) -> Grade {
        let passed_objects = self.hits(GameMode::TKO);
        let mods = self.mods();
        if self.count_300() == passed_objects {
            return if mods.contains(GameMods::Hidden) || mods.contains(GameMods::Flashlight) {
                Grade::XH
            } else {
                Grade::X
            };
        }
        let acc = acc.unwrap_or_else(|| self.acc(GameMode::TKO));
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
        } else {
            Grade::C
        }
    }

    fn ctb_grade(&self, acc: Option<f32>) -> Grade {
        let mods = self.mods();
        let acc = acc.unwrap_or_else(|| self.acc(GameMode::CTB));
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
    fn count_miss(&self) -> u32 {
        self.count_miss
    }
    fn count_50(&self) -> u32 {
        self.count50
    }
    fn count_100(&self) -> u32 {
        self.count100
    }
    fn count_300(&self) -> u32 {
        self.count300
    }
    fn count_geki(&self) -> u32 {
        self.count_geki
    }
    fn count_katu(&self) -> u32 {
        self.count_katu
    }
    fn max_combo(&self) -> u32 {
        self.max_combo
    }
    fn mods(&self) -> GameMods {
        self.enabled_mods
    }
    fn grade(&self, _mode: GameMode) -> Grade {
        self.grade
    }
    fn score(&self) -> u32 {
        self.score
    }
    fn pp(&self) -> Option<f32> {
        self.pp
    }
    fn acc(&self, mode: GameMode) -> f32 {
        self.accuracy(mode)
    }
}

impl ScoreExt for &Score {
    fn count_miss(&self) -> u32 {
        self.count_miss
    }
    fn count_50(&self) -> u32 {
        self.count50
    }
    fn count_100(&self) -> u32 {
        self.count100
    }
    fn count_300(&self) -> u32 {
        self.count300
    }
    fn count_geki(&self) -> u32 {
        self.count_geki
    }
    fn count_katu(&self) -> u32 {
        self.count_katu
    }
    fn max_combo(&self) -> u32 {
        self.max_combo
    }
    fn mods(&self) -> GameMods {
        self.enabled_mods
    }
    fn grade(&self, _mode: GameMode) -> Grade {
        self.grade
    }
    fn score(&self) -> u32 {
        self.score
    }
    fn pp(&self) -> Option<f32> {
        self.pp
    }
    fn acc(&self, mode: GameMode) -> f32 {
        self.accuracy(mode)
    }
}

impl ScoreExt for &OsuStatsScore {
    fn count_miss(&self) -> u32 {
        self.count_miss
    }
    fn count_50(&self) -> u32 {
        self.count50
    }
    fn count_100(&self) -> u32 {
        self.count100
    }
    fn count_300(&self) -> u32 {
        self.count300
    }
    fn count_geki(&self) -> u32 {
        self.count_geki
    }
    fn count_katu(&self) -> u32 {
        self.count_katu
    }
    fn max_combo(&self) -> u32 {
        self.max_combo
    }
    fn mods(&self) -> GameMods {
        self.enabled_mods
    }
    fn hits(&self, _mode: GameMode) -> u32 {
        let mut amount = self.count300 + self.count100 + self.count_miss;
        let mode = self.map.mode;
        if mode != GameMode::TKO {
            amount += self.count50;
            if mode != GameMode::STD {
                amount += self.count_katu;
                if mode != GameMode::CTB {
                    amount += self.count_geki;
                }
            }
        }
        amount
    }
    fn grade(&self, _: GameMode) -> Grade {
        self.grade
    }
    fn score(&self) -> u32 {
        self.score
    }
    fn pp(&self) -> Option<f32> {
        self.pp
    }
    fn acc(&self, _: GameMode) -> f32 {
        self.accuracy
    }
}

impl ScoreExt for &ScraperScore {
    fn count_miss(&self) -> u32 {
        self.count_miss
    }
    fn count_50(&self) -> u32 {
        self.count50
    }
    fn count_100(&self) -> u32 {
        self.count100
    }
    fn count_300(&self) -> u32 {
        self.count300
    }
    fn count_geki(&self) -> u32 {
        self.count_geki
    }
    fn count_katu(&self) -> u32 {
        self.count_katu
    }
    fn max_combo(&self) -> u32 {
        self.max_combo
    }
    fn mods(&self) -> GameMods {
        self.enabled_mods
    }
    fn hits(&self, _: GameMode) -> u32 {
        let mut amount = self.count300 + self.count100 + self.count_miss;
        if self.mode != GameMode::TKO {
            amount += self.count50;
            if self.mode != GameMode::STD {
                amount += self.count_katu;
                if self.mode != GameMode::CTB {
                    amount += self.count_geki;
                }
            }
        }
        amount
    }
    fn grade(&self, _: GameMode) -> Grade {
        self.grade
    }
    fn score(&self) -> u32 {
        self.score
    }
    fn pp(&self) -> Option<f32> {
        self.pp
    }
    fn acc(&self, _: GameMode) -> f32 {
        self.accuracy
    }
}
