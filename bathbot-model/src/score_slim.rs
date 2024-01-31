use bathbot_util::{ScoreExt, ScoreHasEndedAt, ScoreHasMode};
use rosu_v2::{
    model::score::LegacyScoreStatistics,
    prelude::{GameMode, GameMods, Grade, Score},
};
use time::OffsetDateTime;

#[derive(Clone)]
pub struct ScoreSlim {
    pub accuracy: f32,
    pub ended_at: OffsetDateTime,
    pub grade: Grade,
    pub max_combo: u32,
    pub mode: GameMode,
    pub mods: GameMods,
    pub pp: f32,
    pub score: u32,
    pub score_id: Option<u64>,
    pub statistics: LegacyScoreStatistics,
}

impl ScoreSlim {
    pub fn new(score: Score, pp: f32) -> Self {
        Self {
            accuracy: score.accuracy,
            ended_at: score.ended_at,
            grade: score.grade,
            max_combo: score.max_combo,
            mode: score.mode,
            mods: score.mods,
            pp,
            score: score.score,
            score_id: score.legacy_score_id,
            statistics: score.statistics.as_legacy(score.mode),
        }
    }

    pub fn total_hits(&self) -> u32 {
        self.statistics.total_hits(self.mode)
    }

    /// Checks for equality compared to another score.
    /// Note that it is already assumed that both scores come from the same
    /// user.
    pub fn is_eq<S: ScoreHasEndedAt>(&self, score: &S) -> bool {
        (self.ended_at.unix_timestamp() - score.ended_at().unix_timestamp()).abs() <= 2
    }
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
    #[inline] fn mods(&self) -> &GameMods { &self.mods }
    #[inline] fn grade(&self) -> Grade { self.grade }
    #[inline] fn score(&self) -> u32 { self.score }
    #[inline] fn pp(&self) -> Option<f32> { Some(self.pp) }
    #[inline] fn accuracy(&self) -> f32 { self.accuracy }
}

#[rustfmt::skip]
impl ScoreHasMode for ScoreSlim {
    #[inline] fn mode(&self) -> GameMode { self.mode }
}

#[rustfmt::skip]
impl ScoreHasEndedAt for ScoreSlim {
    #[inline] fn ended_at(&self) -> OffsetDateTime { self.ended_at }
}
