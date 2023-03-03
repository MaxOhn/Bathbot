use bathbot_util::{ScoreExt, ScoreHasEndedAt, ScoreHasMode};
use rosu_v2::prelude::{CountryCode, GameMode, GameMods, Grade, RankStatus, Username};
use serde::{Deserialize, Deserializer};
use time::OffsetDateTime;

use super::deser;

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
    pub username: Username,
    pub country_code: CountryCode,
    pub accuracy: f32,
    pub mode: GameMode,
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
            #[serde(with = "deser::adjust_acc")]
            accuracy: f32,
            mods: GameMods,
            #[serde(rename = "total_score")]
            score: u32,
            #[serde(rename = "ruleset_id")]
            mode: GameMode,
            max_combo: u32,
            // #[serde(rename = "legacy_perfect")]
            // perfect: bool,
            statistics: ScraperScoreStatistics,
            pp: Option<f32>,
            rank: Grade,
            #[serde(with = "deser::datetime")]
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
            username: Username,
            country_code: CountryCode,
        }

        let helper = Outer::deserialize(d)?;

        Ok(ScraperScore {
            id: helper.id,
            user_id: helper.user_id,
            username: helper.user.username,
            country_code: helper.user.country_code,
            accuracy: helper.accuracy,
            mode: helper.mode,
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
}

#[rustfmt::skip]
impl ScoreHasMode for ScraperScore {
    #[inline] fn mode(&self) -> GameMode { self.mode }
}

#[rustfmt::skip]
impl ScoreHasEndedAt for ScraperScore {
    #[inline] fn ended_at(&self) -> OffsetDateTime { self.date }
}

#[derive(Deserialize)]
pub struct ScraperBeatmap {
    pub id: u32,
    pub beatmapset_id: u32,
    #[serde(rename = "mode_int")]
    pub mode: GameMode,
    pub difficulty_rating: f32,
    pub version: Box<str>,
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
    #[serde(with = "deser::datetime")]
    pub last_updated: OffsetDateTime,
    pub ranked: RankStatus,
}
