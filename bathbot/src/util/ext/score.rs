use bathbot_model::{OsuStatsScore, ScoreSlim, ScraperScore};
use bathbot_util::osu::calculate_grade;
use rosu_pp::ScoreState;
use rosu_v2::prelude::{GameMode, GameMods, Grade, MatchScore, Score, ScoreStatistics};

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
    fn accuracy(&self) -> f32;
    fn mode(&self) -> Option<GameMode>;

    // Optional to implement
    #[inline]
    fn grade(&self, mode: GameMode) -> Grade {
        let stats = ScoreStatistics {
            count_geki: self.count_geki(),
            count_300: self.count_300(),
            count_katu: self.count_katu(),
            count_100: self.count_100(),
            count_50: self.count_50(),
            count_miss: self.count_miss(),
        };

        calculate_grade(mode, self.mods(), &stats)
    }

    #[inline]
    fn total_hits(&self, mode: u8) -> u32 {
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
            n_misses: self.count_miss() as usize,
            n_geki: self.count_geki() as usize,
            n300: self.count_300() as usize,
            n_katu: self.count_katu() as usize,
            n100: self.count_100() as usize,
            n50: self.count_50() as usize,
        }
    }
}

#[rustfmt::skip]
impl ScoreExt for Score {
    #[inline] fn count_miss(&self) -> u32 { self.statistics.count_miss }
    #[inline] fn count_50(&self) -> u32 { self.statistics.count_50 }
    #[inline] fn count_100(&self) -> u32 { self.statistics.count_100 }
    #[inline] fn count_300(&self) -> u32 { self.statistics.count_300 }
    #[inline] fn count_geki(&self) -> u32 { self.statistics.count_geki }
    #[inline] fn count_katu(&self) -> u32 { self.statistics.count_katu }
    #[inline] fn max_combo(&self) -> u32 { self.max_combo }
    #[inline] fn mods(&self) -> GameMods { self.mods }
    #[inline] fn grade(&self, _: GameMode) -> Grade { self.grade }
    #[inline] fn score(&self) -> u32 { self.score }
    #[inline] fn pp(&self) -> Option<f32> { self.pp }
    #[inline] fn accuracy(&self) -> f32 { self.accuracy }
    #[inline] fn mode(&self) -> Option<GameMode> { Some(self.mode) }
}

#[rustfmt::skip]
impl ScoreExt for ScoreSlim {
    #[inline] fn count_miss(&self) -> u32 { self.statistics.count_miss }
    #[inline] fn count_50(&self) -> u32 { self.statistics.count_50 }
    #[inline] fn count_100(&self) -> u32 { self.statistics.count_100 }
    #[inline] fn count_300(&self) -> u32 { self.statistics.count_300 }
    #[inline] fn count_geki(&self) -> u32 { self.statistics.count_geki }
    #[inline] fn count_katu(&self) -> u32 { self.statistics.count_katu }
    #[inline] fn max_combo(&self) -> u32 { self.max_combo }
    #[inline] fn mods(&self) -> GameMods { self.mods }
    #[inline] fn grade(&self, _: GameMode) -> Grade { self.grade }
    #[inline] fn score(&self) -> u32 { self.score }
    #[inline] fn pp(&self) -> Option<f32> { Some(self.pp) }
    #[inline] fn accuracy(&self) -> f32 { self.accuracy }
    #[inline] fn mode(&self) -> Option<GameMode> { Some(self.mode) }
}

#[rustfmt::skip]
impl ScoreExt for OsuStatsScore {
    #[inline] fn count_miss(&self) -> u32 { self.count_miss }
    #[inline] fn count_50(&self) -> u32 { self.count50 }
    #[inline] fn count_100(&self) -> u32 { self.count100 }
    #[inline] fn count_300(&self) -> u32 { self.count300 }
    #[inline] fn count_geki(&self) -> u32 { self.count_geki }
    #[inline] fn count_katu(&self) -> u32 { self.count_katu }
    #[inline] fn max_combo(&self) -> u32 { self.max_combo }
    #[inline] fn mods(&self) -> GameMods { self.mods }
    #[inline] fn grade(&self, _: GameMode) -> Grade { self.grade }
    #[inline] fn score(&self) -> u32 { self.score }
    #[inline] fn pp(&self) -> Option<f32> { self.pp }
    #[inline] fn accuracy(&self) -> f32 { self.accuracy }
    // self.map.mode is the map's original mode, not converted
    #[inline] fn mode(&self) -> Option<GameMode> { None }
}

#[rustfmt::skip]
impl ScoreExt for ScraperScore {
    #[inline] fn count_miss(&self) -> u32 { self.count_miss }
    #[inline] fn count_50(&self) -> u32 { self.count50 }
    #[inline] fn count_100(&self) -> u32 { self.count100 }
    #[inline] fn count_300(&self) -> u32 { self.count300 }
    #[inline] fn count_geki(&self) -> u32 { self.count_geki }
    #[inline] fn count_katu(&self) -> u32 { self.count_katu }
    #[inline] fn max_combo(&self) -> u32 { self.max_combo }
    #[inline] fn mods(&self) -> GameMods { self.mods }
    #[inline] fn grade(&self, _: GameMode) -> Grade { self.grade }
    #[inline] fn score(&self) -> u32 { self.score }
    #[inline] fn pp(&self) -> Option<f32> { self.pp }
    #[inline] fn accuracy(&self) -> f32 { self.accuracy }
    #[inline] fn mode(&self) -> Option<GameMode> { Some(self.mode) }
}

#[rustfmt::skip]
impl ScoreExt for MatchScore {
    #[inline] fn count_miss(&self) -> u32 { self.statistics.count_miss }
    #[inline] fn count_50(&self) -> u32 { self.statistics.count_50 }
    #[inline] fn count_100(&self) -> u32 { self.statistics.count_100 }
    #[inline] fn count_300(&self) -> u32 { self.statistics.count_300 }
    #[inline] fn count_geki(&self) -> u32 { self.statistics.count_geki }
    #[inline] fn count_katu(&self) -> u32 { self.statistics.count_katu }
    #[inline] fn max_combo(&self) -> u32 { self.max_combo }
    #[inline] fn mods(&self) -> GameMods { self.mods }
    #[inline] fn score(&self) -> u32 { self.score }
    #[inline] fn pp(&self) -> Option<f32> { None }
    #[inline] fn accuracy(&self) -> f32 { self.accuracy }
    #[inline] fn mode(&self) -> Option<GameMode> { None }
}
