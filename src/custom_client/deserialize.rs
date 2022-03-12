use crate::util::constants::DATE_FORMAT;

use chrono::{offset::TimeZone, DateTime, Utc};
use rosu_v2::model::GameMods;
use serde::{
    de::{Error, Unexpected, Visitor},
    Deserialize, Deserializer,
};
use std::{fmt, str::FromStr};

pub fn str_to_maybe_datetime<'de, D>(d: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    d.deserialize_option(MaybeDateTimeString)
}

struct MaybeDateTimeString;

impl<'de> Visitor<'de> for MaybeDateTimeString {
    type Value = Option<DateTime<Utc>>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string containing a datetime")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        match Utc.datetime_from_str(v, DATE_FORMAT) {
            Ok(date) => Ok(Some(date)),
            Err(_) => Ok(None),
        }
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_str(self)
    }

    fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
}

pub fn str_to_datetime<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
    Ok(str_to_maybe_datetime(d)?.unwrap())
}

pub fn str_to_maybe_f32<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f32>, D::Error> {
    d.deserialize_option(MaybeF32String)
}

struct MaybeF32String;

impl<'de> Visitor<'de> for MaybeF32String {
    type Value = Option<f32>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string containing an f32")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        v.parse()
            .map(Some)
            .map_err(|_| Error::invalid_value(Unexpected::Str(v), &self))
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_str(self)
    }

    fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
}

pub fn str_to_f32<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    Ok(str_to_maybe_f32(d)?.unwrap_or(0.0))
}

pub fn str_to_maybe_u32<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    d.deserialize_option(MaybeU32String)
}

struct MaybeU32String;

impl<'de> Visitor<'de> for MaybeU32String {
    type Value = Option<u32>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string containing an u32")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        v.parse()
            .map(Some)
            .map_err(|_| Error::invalid_value(Unexpected::Str(v), &self))
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_str(self)
    }

    fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
}

pub fn str_to_u32<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    Ok(str_to_maybe_u32(d)?.unwrap_or(0))
}

pub fn adjust_mods_maybe<'de, D: Deserializer<'de>>(d: D) -> Result<Option<GameMods>, D::Error> {
    d.deserialize_option(MaybeModsString)
}

struct MaybeModsString;

impl<'de> Visitor<'de> for MaybeModsString {
    type Value = Option<GameMods>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string containing gamemods")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        let mut mods = GameMods::NoMod;

        if v == "None" {
            return Ok(Some(mods));
        }

        for result in v.split(',').map(GameMods::from_str) {
            match result {
                Ok(m) => mods |= m,
                Err(why) => {
                    return Err(Error::custom(format_args!(r#"invalid value "{v}": {why}"#)));
                }
            }
        }

        Ok(Some(mods))
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_str(self)
    }

    fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
}

pub fn adjust_mods<'de, D: Deserializer<'de>>(d: D) -> Result<GameMods, D::Error> {
    Ok(adjust_mods_maybe(d)?.unwrap_or_default())
}

pub fn expect_negative_u32<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let i: i64 = Deserialize::deserialize(d)?;

    Ok(i.max(0) as u32)
}

pub fn inflate_acc<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    let acc: f32 = Deserialize::deserialize(d)?;

    Ok(100.0 * acc)
}
