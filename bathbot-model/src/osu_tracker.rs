use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use rosu_v2::prelude::{CountryCode, GameModsIntermode, Username};
use serde::Deserialize;
use time::OffsetDateTime;

use crate::rkyv_util::DerefAsString;

use super::deser;

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

#[derive(Debug, Deserialize)]
pub struct OsuTrackerCountryDetails {
    pub contributors: Vec<OsuTrackerCountryContributor>,
    #[serde(rename = "scoresCurrent")]
    pub scores: Vec<OsuTrackerCountryScore>,
    #[serde(rename = "name")]
    pub country: Box<str>,
    #[serde(rename = "abbreviation")]
    pub code: CountryCode,
    #[serde(with = "deser::f32_string")]
    pub pp: f32,
    // #[serde(with = "deser::f32_string")]
    // pub range: f32,
    #[serde(with = "deser::adjust_acc")]
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
    pub name: Box<str>,
    #[serde(rename = "id", with = "deser::u32_string")]
    pub map_id: u32,
    #[serde(rename = "setId", with = "deser::u32_string")]
    pub mapset_id: u32,
    pub mods: GameModsIntermode,
    #[serde(with = "deser::f32_string")]
    pub pp: f32,
    #[serde(rename = "missCount", with = "deser::u32_string")]
    pub n_misses: u32,
    #[serde(with = "deser::adjust_acc")]
    pub acc: f32,
    #[serde(rename = "length", with = "deser::u32_string")]
    pub seconds_total: u32,
    pub mapper: Username,
    #[serde(rename = "time", with = "maybe_naive_datetime")]
    pub ended_at: OffsetDateTime,
    pub player: Username,
}

pub(super) mod maybe_naive_datetime {
    use std::fmt::{Formatter, Result as FmtResult};

    use bathbot_util::datetime::{DATE_FORMAT, TIME_FORMAT};
    use serde::{
        de::{Error, Visitor},
        Deserializer,
    };
    use time::{Date, OffsetDateTime, PrimitiveDateTime, Time};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<OffsetDateTime, D::Error> {
        d.deserialize_str(DateTimeVisitor)
    }

    pub(super) struct DateTimeVisitor;

    impl<'de> Visitor<'de> for DateTimeVisitor {
        type Value = OffsetDateTime;

        fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
            f.write_str("a datetime string")
        }

        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            if v.len() < 10 {
                return Err(Error::custom(format!("string too short for a date: `{v}`")));
            }

            let (prefix, suffix) = v.split_at(10);

            let date = Date::parse(prefix, DATE_FORMAT).map_err(Error::custom)?;

            let time_bytes = match suffix.as_bytes() {
                [] => return Err(Error::custom(format!("string too short for a time: `{v}`"))),
                [b'T', infix @ .., b'Z'] => infix,
                [b' ', suffix @ ..] => suffix,
                _ => return Err(Error::custom(format!("invalid time format: `{v}`"))),
            };

            // SAFETY: The slice originates from a str in the first place
            let time_str = unsafe { std::str::from_utf8_unchecked(time_bytes) };
            let time = Time::parse(time_str, TIME_FORMAT).map_err(Error::custom)?;

            Ok(PrimitiveDateTime::new(date, time).assume_utc())
        }
    }
}
