use rosu_v2::prelude::{BeatmapUserScore, GameMode, GameMods, Grade, Score, ScoreStatistics};
use time::OffsetDateTime;

use crate::osu::calculate_grade;

pub trait ScoreExt {
    // Required to implement
    fn count_miss(&self) -> u32;
    fn count_50(&self) -> u32;
    fn count_100(&self) -> u32;
    fn count_300(&self) -> u32;
    fn count_geki(&self) -> u32;
    fn count_katu(&self) -> u32;
    fn max_combo(&self) -> u32;
    fn mods(&self) -> &GameMods;
    fn score(&self) -> u32;
    fn pp(&self) -> Option<f32>;
    fn accuracy(&self) -> f32;

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
    #[inline] fn mods(&self) -> &GameMods { &self.mods }
    #[inline] fn grade(&self, _: GameMode) -> Grade { self.grade }
    #[inline] fn score(&self) -> u32 { self.score }
    #[inline] fn pp(&self) -> Option<f32> { self.pp }
    #[inline] fn accuracy(&self) -> f32 { self.accuracy }
}

// TODO
// #[rustfmt::skip]
// impl ScoreExt for MatchScore {
//     #[inline] fn count_miss(&self) -> u32 { self.statistics.count_miss }
//     #[inline] fn count_50(&self) -> u32 { self.statistics.count_50 }
//     #[inline] fn count_100(&self) -> u32 { self.statistics.count_100 }
//     #[inline] fn count_300(&self) -> u32 { self.statistics.count_300 }
//     #[inline] fn count_geki(&self) -> u32 { self.statistics.count_geki }
//     #[inline] fn count_katu(&self) -> u32 { self.statistics.count_katu }
//     #[inline] fn max_combo(&self) -> u32 { self.max_combo }
//     #[inline] fn mods(&self) -> GameMods { self.mods }
//     #[inline] fn score(&self) -> u32 { self.score }
//     #[inline] fn pp(&self) -> Option<f32> { None }
//     #[inline] fn accuracy(&self) -> f32 { self.accuracy }
// }

pub trait ScoreHasMode {
    fn mode(&self) -> GameMode;
}

#[rustfmt::skip]
impl ScoreHasMode for Score {
    #[inline] fn mode(&self) -> GameMode { self.mode }
}

pub trait ScoreHasEndedAt {
    fn ended_at(&self) -> OffsetDateTime;
}

#[rustfmt::skip]
impl ScoreHasEndedAt for Score {
    #[inline] fn ended_at(&self) -> OffsetDateTime { self.ended_at }
}

#[rustfmt::skip]
impl ScoreHasEndedAt for BeatmapUserScore {
    #[inline] fn ended_at(&self) -> OffsetDateTime { self.score.ended_at }
}
