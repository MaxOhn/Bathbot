use rosu_v2::prelude::GameMods;
use serde::Deserialize;

use super::deserialize::{inflate_acc, str_to_u32};

#[derive(Debug, Deserialize)]
pub struct OsuTrackerPpGroup {
    pub number: u32,
    pub list: Vec<OsuTrackerPpEntry>,
}

#[derive(Debug, Deserialize)]
pub struct OsuTrackerPpEntry {
    pub name: String,
    #[serde(rename = "id", deserialize_with = "str_to_u32")]
    pub map_id: u32,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
pub struct OsuTrackerStats {
    #[serde(rename = "userStats")]
    pub user: OsuTrackerUserStats,
    #[serde(rename = "countryStats")]
    pub country: OsuTrackerCountryStats,
    #[serde(rename = "mapperCount")]
    pub mapper_count: Vec<OsuTrackerMapperEntry>,
    #[serde(rename = "setCount")]
    pub mapset_count: Vec<OsuTrackerMapsetEntry>,
}

#[derive(Debug, Deserialize)]
pub struct OsuTrackerUserStats {
    pub range: f32,
    pub acc: f32,
    pub plays: f32,
    pub farm: f32,
    pub pp: f32,
    pub level: f32,
    #[serde(rename = "lengthPlay")]
    pub length_play: f32,
    #[serde(rename = "objectsPlay")]
    pub objects_play: f32,
    #[serde(rename = "modsCount")]
    pub mods_count: Vec<OsuTrackerModsEntry>,
    // #[serde(rename = "topPlay", deserialize_with = "str_to_u32")]
    // top_play: u32,
}

#[derive(Debug, Deserialize)]
pub struct OsuTrackerCountryStats {
    #[serde(deserialize_with = "inflate_acc")]
    pub acc: f32,
    pub farm: f32,
    #[serde(rename = "lengthPlay")]
    pub length_play: f32,
    // #[serde(rename = "modsCount")]
    // mods_count: OsuTrackerModsEntry,
    #[serde(rename = "objectsPlay")]
    pub objects_play: f32,
    pub pp: f32,
    pub range: f32,
}

#[derive(Copy, Clone, Debug, Deserialize)]
pub struct OsuTrackerModsEntry {
    pub mods: GameMods,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
pub struct OsuTrackerMapperEntry {
    pub mapper: String,
    pub count: usize,
}

#[derive(Copy, Clone, Debug, Deserialize)]
pub struct OsuTrackerMapsetEntry {
    #[serde(rename = "setId", deserialize_with = "str_to_u32")]
    pub mapset_id: u32,
    pub count: usize,
}
