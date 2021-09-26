use crate::util::CountryCode;

use chrono::{DateTime, Utc};
use rosu_v2::prelude::{GameMode, GameMods, Grade, RankStatus};
use serde::{de, Deserialize, Deserializer};

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
    pub perfect: bool,
    pub pp: Option<f32>,
    pub grade: Grade,
    pub date: DateTime<Utc>,
    pub mode: GameMode,
    pub replay: bool,
    pub beatmap: ScraperBeatmap,
    pub count50: u32,
    pub count100: u32,
    pub count300: u32,
    pub count_geki: u32,
    pub count_katu: u32,
    pub count_miss: u32,
}

impl<'de> Deserialize<'de> for ScraperScore {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Outer {
            id: u64,
            user_id: u32,
            #[serde(deserialize_with = "adjust_acc")]
            accuracy: f32,
            mods: GameMods,
            score: u32,
            max_combo: u32,
            perfect: bool,
            statistics: ScraperScoreStatistics,
            pp: Option<f32>,
            rank: Grade,
            #[serde(deserialize_with = "adjust_datetime")]
            created_at: DateTime<Utc>,
            mode_int: GameMode,
            replay: bool,
            beatmap: ScraperBeatmap,
            user: ScraperUser,
        }

        #[derive(Deserialize)]
        pub struct ScraperScoreStatistics {
            #[serde(default)]
            count_50: u32,
            #[serde(default)]
            count_100: u32,
            #[serde(default)]
            count_300: u32,
            #[serde(default)]
            count_geki: u32,
            #[serde(default)]
            count_katu: u32,
            #[serde(default)]
            count_miss: u32,
        }

        #[derive(Deserialize)]
        pub struct ScraperUser {
            username: String,
            country_code: String,
        }

        let helper = Outer::deserialize(deserializer)?;

        Ok(ScraperScore {
            id: helper.id,
            user_id: helper.user_id,
            username: helper.user.username,
            country_code: helper.user.country_code,
            accuracy: helper.accuracy,
            mods: helper.mods,
            score: helper.score,
            max_combo: helper.max_combo,
            perfect: helper.perfect,
            pp: helper.pp,
            grade: helper.rank,
            date: helper.created_at,
            mode: helper.mode_int,
            replay: helper.replay,
            beatmap: helper.beatmap,
            count50: helper.statistics.count_50,
            count100: helper.statistics.count_100,
            count300: helper.statistics.count_300,
            count_geki: helper.statistics.count_geki,
            count_katu: helper.statistics.count_katu,
            count_miss: helper.statistics.count_miss,
        })
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
    #[serde(deserialize_with = "adjust_datetime")]
    pub last_updated: DateTime<Utc>,
    pub ranked: RankStatus,
}

fn adjust_acc<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    let f: f32 = Deserialize::deserialize(d)?;

    Ok(f * 100.0)
}

fn adjust_datetime<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
    let d: &str = Deserialize::deserialize(d)?;

    let d = DateTime::parse_from_rfc3339(d)
        .map_err(de::Error::custom)?
        .with_timezone(&Utc);

    Ok(d)
}
