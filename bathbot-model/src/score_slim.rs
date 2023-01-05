use rosu_v2::prelude::{GameMode, GameMods, Grade, Score, ScoreStatistics};
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
    pub statistics: ScoreStatistics,
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
            score_id: score.score_id,
            statistics: score.statistics,
        }
    }

    pub fn total_hits(&self) -> u32 {
        self.statistics.total_hits(self.mode)
    }
}
