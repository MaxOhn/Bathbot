use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use rosu_v2::prelude::{GameModsIntermode, Username};
use serde::Deserialize;

use super::deser;
use crate::rkyv_util::DerefAsString;

#[derive(Archive, Copy, Clone, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsuTrackerIdCount {
    #[serde(rename = "id")]
    pub map_id: u32,
    pub count: usize,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsuTrackerPpGroup {
    pub number: u32,
    pub list: Vec<OsuTrackerPpEntry>,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsuTrackerPpEntry {
    #[with(DerefAsString)]
    pub name: Username,
    #[serde(rename = "id", with = "deser::u32_string")]
    pub map_id: u32,
    pub count: usize,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
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

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
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
    pub mods_count: Box<[OsuTrackerModsEntry]>,
    // #[serde(rename = "topPlay", with = "u32_string")]
    // top_play: u32,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsuTrackerCountryStats {
    #[serde(with = "deser::adjust_acc")]
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

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsuTrackerModsEntry {
    pub mods: GameModsIntermode,
    pub count: usize,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsuTrackerMapperEntry {
    #[with(DerefAsString)]
    pub mapper: Username,
    pub count: usize,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsuTrackerMapsetEntry {
    #[serde(rename = "setId", with = "deser::u32_string")]
    pub mapset_id: u32,
    pub count: usize,
}
