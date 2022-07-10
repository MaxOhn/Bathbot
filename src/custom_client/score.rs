use rosu_pp::ScoreState;
use rosu_v2::prelude::{GameMode, GameMods, Grade, RankStatus};
use serde::{Deserialize, Deserializer};
use time::OffsetDateTime;

use crate::util::CountryCode;

use super::deserialize;

#[derive(Deserialize)]
pub struct ScraperScores {
    scores: Vec<ScraperScore>,
}

impl ScraperScores {
    pub fn get(self) -> Vec<ScraperScore> {
        self.scores
    }
}

pub struct ScraperScore {
    pub id: u64,
    pub user_id: u32,
    pub username: String,
    pub country_code: CountryCode,
    pub accuracy: f32,
    pub mods: GameMods,
    pub score: u32,
    pub max_combo: u32,
    // pub perfect: bool,
    pub pp: Option<f32>,
    pub grade: Grade,
    pub date: OffsetDateTime,
    pub replay: bool,
    pub count50: u32,
    pub count100: u32,
    pub count300: u32,
    pub count_geki: u32,
    pub count_katu: u32,
    pub count_miss: u32,
}

impl<'de> Deserialize<'de> for ScraperScore {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Outer {
            id: u64,
            user_id: u32,
            #[serde(with = "deserialize::adjust_acc")]
            accuracy: f32,
            mods: GameMods,
            #[serde(rename = "total_score")]
            score: u32,
            max_combo: u32,
            // #[serde(rename = "legacy_perfect")]
            // perfect: bool,
            statistics: ScraperScoreStatistics,
            pp: Option<f32>,
            rank: Grade,
            #[serde(with = "deserialize::offset_datetime")]
            ended_at: OffsetDateTime,
            replay: bool,
            user: ScraperUser,
        }

        #[derive(Deserialize)]
        pub struct ScraperScoreStatistics {
            #[serde(default, rename = "perfect")]
            count_geki: u32,
            #[serde(default, rename = "good", alias = "small_tick_miss")]
            count_katu: u32,
            #[serde(default, rename = "great")]
            count_300: u32,
            #[serde(default, rename = "ok", alias = "large_tick_hit")]
            count_100: u32,
            #[serde(default, rename = "meh", alias = "small_tick_hit")]
            count_50: u32,
            #[serde(default, rename = "miss")]
            count_miss: u32,
        }

        #[derive(Deserialize)]
        pub struct ScraperUser {
            username: String,
            country_code: CountryCode,
        }

        let helper = Outer::deserialize(d)?;

        Ok(ScraperScore {
            id: helper.id,
            user_id: helper.user_id,
            username: helper.user.username,
            country_code: helper.user.country_code,
            accuracy: helper.accuracy,
            mods: helper.mods,
            score: helper.score,
            max_combo: helper.max_combo,
            // perfect: helper.perfect,
            pp: helper.pp,
            grade: helper.rank,
            date: helper.ended_at,
            replay: helper.replay,
            count50: helper.statistics.count_50,
            count100: helper.statistics.count_100,
            count300: helper.statistics.count_300,
            count_geki: helper.statistics.count_geki,
            count_katu: helper.statistics.count_katu,
            count_miss: helper.statistics.count_miss,
        })
    }
}

impl From<&ScraperScore> for ScoreState {
    #[inline]
    fn from(score: &ScraperScore) -> Self {
        ScoreState {
            max_combo: score.max_combo as usize,
            n300: score.count300 as usize,
            n100: score.count100 as usize,
            n50: score.count50 as usize,
            n_katu: score.count_katu as usize,
            misses: score.count_miss as usize,
            score: score.score,
        }
    }
}

#[derive(Deserialize)]
pub struct ScraperBeatmap {
    pub id: u32,
    pub beatmapset_id: u32,
    #[serde(rename = "mode_int")]
    pub mode: GameMode,
    pub difficulty_rating: f32,
    pub version: String,
    pub total_length: u32,
    pub hit_length: u32,
    pub bpm: f32,
    pub cs: f32,
    #[serde(rename = "drain")]
    pub hp: f32,
    #[serde(rename = "accuracy")]
    pub od: f32,
    pub ar: f32,
    #[serde(default)]
    pub playcount: u32,
    #[serde(default)]
    pub passcount: u32,
    #[serde(default)]
    pub count_circles: u32,
    #[serde(default)]
    pub count_sliders: u32,
    #[serde(default)]
    pub count_spinner: u32,
    #[serde(default)]
    pub count_total: u32,
    #[serde(with = "deserialize::offset_datetime")]
    pub last_updated: OffsetDateTime,
    pub ranked: RankStatus,
}
