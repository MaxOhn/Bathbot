use super::deserialize::{adjust_mods_maybe, str_to_f32, str_to_maybe_u32, str_to_u32};

use rosu_v2::model::{GameMode, GameMods};
use serde::{
    de::{Error, Unexpected},
    Deserialize, Deserializer,
};

#[derive(Deserialize)]
pub struct OsekaiMedal {
    #[serde(rename = "medalid", deserialize_with = "str_to_maybe_u32")]
    pub medal_id: Option<u32>,
    pub name: String,
    #[serde(rename = "link")]
    pub url: String,
    pub description: String,
    #[serde(rename = "restriction", deserialize_with = "str_to_maybe_mode")]
    pub mode: Option<GameMode>,
    #[serde(rename = "grouping")]
    pub group: String,
    pub solution: Option<String>,
    #[serde(deserialize_with = "adjust_mods_maybe")]
    pub mods: Option<GameMods>,
    #[serde(default)]
    pub difficulty: Option<OsekaiDifficulty>,
    #[serde(default)]
    pub beatmaps: Vec<OsekaiMap>,
    #[serde(default)]
    pub comments: Vec<OsekaiComment>,
}

#[derive(Deserialize)]
pub struct OsekaiMap {
    #[serde(rename = "BeatmapID", deserialize_with = "str_to_u32")]
    pub beatmap_id: u32,
    #[serde(rename = "MapsetID", deserialize_with = "str_to_u32")]
    pub beatmapset_id: u32,
    #[serde(rename = "Gamemode")]
    pub mode: GameMode,
    #[serde(rename = "SongTitle")]
    pub title: String,
    #[serde(rename = "Artist")]
    pub artist: String,
    #[serde(rename = "DifficultyName")]
    pub version: String,
    #[serde(rename = "Mapper")]
    pub creator: String,
    #[serde(deserialize_with = "str_to_f32")]
    pub bpm: f32,
    #[serde(rename = "Difficulty", deserialize_with = "str_to_f32")]
    pub stars: f32,
}

#[derive(Deserialize)]
pub struct OsekaiDifficulty {
    #[serde(rename = "TotalDedication", deserialize_with = "str_to_f32")]
    pub dedication: f32,
    #[serde(rename = "TotalTapping", deserialize_with = "str_to_f32")]
    pub tapping: f32,
    #[serde(rename = "TotalReading", deserialize_with = "str_to_f32")]
    pub reading: f32,
    #[serde(rename = "TotalPatterns", deserialize_with = "str_to_f32")]
    pub patterns: f32,
    #[serde(rename = "TotalScore", deserialize_with = "str_to_f32")]
    pub total: f32,
    #[serde(rename = "VotingCount", deserialize_with = "str_to_u32")]
    pub voting_count: u32,
}

#[derive(Debug, Deserialize)]
pub struct OsekaiComment {
    #[serde(rename = "ID", deserialize_with = "str_to_u32")]
    pub comment_id: u32,
    #[serde(rename = "PostText")]
    pub content: String,
    #[serde(rename = "Username")]
    pub username: String,
    #[serde(rename = "voteSum", deserialize_with = "str_to_u32")]
    pub vote_sum: u32,
    #[serde(rename = "ParentComment", deserialize_with = "str_to_maybe_u32")]
    pub parent_id: Option<u32>,
}

fn str_to_maybe_mode<'de, D>(d: D) -> Result<Option<GameMode>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Deserialize::deserialize(d)?;

    let m = match s.as_deref() {
        Some("NULL") | None => return Ok(None),
        Some("osu") => GameMode::STD,
        Some("taiko") => GameMode::TKO,
        Some("fruits") => GameMode::CTB,
        Some("mania") => GameMode::MNA,
        Some(s) => {
            return Err(Error::invalid_value(
                Unexpected::Str(s),
                &r#""osu", "taiko", "fruits", "mania", "NULL", or null"#,
            ))
        }
    };

    Ok(Some(m))
}
