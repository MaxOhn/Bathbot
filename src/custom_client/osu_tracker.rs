use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use rosu_v2::prelude::{CountryCode, GameMods, Username};
use serde::Deserialize;
use time::OffsetDateTime;

use super::{deserialize, UsernameWrapper};

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
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
    pub name: String,
    #[serde(rename = "id", with = "deserialize::u32_string")]
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
    pub mods_count: Vec<OsuTrackerModsEntry>,
    // #[serde(rename = "topPlay", with = "u32_string")]
    // top_play: u32,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsuTrackerCountryStats {
    #[serde(with = "deserialize::adjust_acc")]
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
    pub mods: GameMods,
    pub count: usize,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsuTrackerMapperEntry {
    #[with(UsernameWrapper)]
    pub mapper: Username,
    pub count: usize,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsuTrackerMapsetEntry {
    #[serde(rename = "setId", with = "deserialize::u32_string")]
    pub mapset_id: u32,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
pub struct OsuTrackerCountryDetails {
    pub contributors: Vec<OsuTrackerCountryContributor>,
    #[serde(rename = "scoresCurrent")]
    pub scores: Vec<OsuTrackerCountryScore>,
    #[serde(rename = "name")]
    pub country: String,
    #[serde(rename = "abbreviation")]
    pub code: CountryCode,
    #[serde(with = "deserialize::f32_string")]
    pub pp: f32,
    // #[serde(with = "deserialize::f32_string")]
    // pub range: f32,
    #[serde(with = "deserialize::adjust_acc")]
    pub acc: f32,
    pub farm: f32,
    #[serde(rename = "averageLength")]
    pub avg_len: f32,
    #[serde(rename = "averageObjects")]
    pub avg_objects: f32,
    // #[serde(rename = "modsCount")]
    // pub mods_count: Vec<OsuTrackerModsEntry>,
}

#[derive(Debug, Deserialize)]
pub struct OsuTrackerCountryContributor {
    pub name: Username,
    pub pp: f32,
}

#[derive(Debug, Deserialize)]
pub struct OsuTrackerCountryScore {
    pub name: String,
    #[serde(rename = "id", with = "deserialize::u32_string")]
    pub map_id: u32,
    #[serde(rename = "setId", with = "deserialize::u32_string")]
    pub mapset_id: u32,
    pub mods: GameMods,
    #[serde(with = "deserialize::f32_string")]
    pub pp: f32,
    #[serde(rename = "missCount", with = "deserialize::u32_string")]
    pub n_misses: u32,
    #[serde(with = "deserialize::adjust_acc")]
    pub acc: f32,
    #[serde(rename = "length", with = "deserialize::u32_string")]
    pub seconds_total: u32,
    pub mapper: Username,
    #[serde(rename = "time", with = "deserialize::offset_datetime")]
    pub ended_at: OffsetDateTime,
    pub player: Username,
}
