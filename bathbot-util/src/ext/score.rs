use rosu_v2::prelude::{BeatmapUserScore, GameMode, GameMods, Grade, Score};
use time::OffsetDateTime;

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
    fn grade(&self) -> Grade;

    /// Returns the *new* kind of score id.
    fn score_id(&self) -> Option<u64>;

    /// Whether the score contains legacy data.
    ///
    /// Note that this does *not* mean whether the score was set on stable,
    /// but whether the score was set on stable *and* was request as legacy
    /// data.
    fn is_legacy(&self) -> bool;

    // Optional to implement
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
            _ if self.count_miss() > 0 || self.grade() == Grade::F => false,
            // Allow 1 missed sliderend per 500 combo
            GameMode::Osu => self.max_combo() >= (max_combo - (max_combo / 500).max(4)),
            GameMode::Taiko | GameMode::Mania => true,
            GameMode::Catch => self.max_combo() == max_combo,
        }
    }
}

#[rustfmt::skip]
impl ScoreExt for Score {
    fn count_miss(&self) -> u32 {
        self.statistics.miss
    }

    fn count_50(&self) -> u32 {
        match self.mode {
            GameMode::Osu => self.statistics.meh,
            GameMode::Taiko => 0,
            GameMode::Catch => self.statistics.small_tick_hit,
            GameMode::Mania => self.statistics.meh,
        }
    }

    fn count_100(&self) -> u32 {
        match self.mode {
            GameMode::Osu => self.statistics.ok,
            GameMode::Taiko => self.statistics.ok,
            GameMode::Catch => self.statistics.large_tick_hit,
            GameMode::Mania => self.statistics.ok,
        }
    }

    fn count_300(&self) -> u32 {
        self.statistics.great
    }

    fn count_geki(&self) -> u32 {
        match self.mode {
            GameMode::Osu => 0,
            GameMode::Taiko => 0,
            GameMode::Catch => 0,
            GameMode::Mania => self.statistics.perfect,
        }
    }

    fn count_katu(&self) -> u32 {
        match self.mode {
            GameMode::Osu => 0,
            GameMode::Taiko => 0,
            GameMode::Catch => self.statistics.small_tick_miss,
            GameMode::Mania => self.statistics.good,
        }
    }

    #[inline] fn max_combo(&self) -> u32 { self.max_combo }
    #[inline] fn mods(&self) -> &GameMods { &self.mods }

    #[inline]
    fn grade(&self) -> Grade {
        if self.passed {
            self.grade
        } else {
            Grade::F
        }
    }

    #[inline] fn score(&self) -> u32 { self.score }
    #[inline] fn pp(&self) -> Option<f32> { self.pp }
    #[inline] fn accuracy(&self) -> f32 { self.accuracy }
    #[inline] fn score_id(&self) -> Option<u64> { Some(self.id) }

    fn is_legacy(&self) -> bool {
        self.legacy_score_id == Some(self.id)
    }
}

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
