use chrono::{offset::TimeZone, DateTime, Utc};
use rosu_v2::model::GameMods;
use serde::{
    de::{Error, Unexpected},
    Deserialize, Deserializer,
};
use std::str::FromStr;

pub fn str_to_maybe_datetime<'de, D>(d: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Deserialize::deserialize(d)?;

    s.map(|s| Utc.datetime_from_str(s.as_str(), "%F %T").map_err(|_| s))
        .transpose()
        .map_err(|s| {
            Error::invalid_value(Unexpected::Str(s.as_str()), &r#"null or datetime "%F %T""#)
        })
}

pub fn str_to_datetime<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
    Ok(str_to_maybe_datetime(d)?.unwrap())
}

pub fn str_to_maybe_f32<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f32>, D::Error> {
    let s: Option<String> = Deserialize::deserialize(d)?;

    s.map(|s| f32::from_str(s.as_str()).map_err(|_| s))
        .transpose()
        .map_err(|s| Error::invalid_value(Unexpected::Str(s.as_str()), &"f32 or null"))
}

pub fn str_to_f32<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    Ok(str_to_maybe_f32(d)?.unwrap_or(0.0))
}

pub fn str_to_maybe_u32<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    let s: Option<String> = Deserialize::deserialize(d)?;
    s.map(|s| u32::from_str(s.as_str()).map_err(|_| s))
        .transpose()
        .map_err(|s| Error::invalid_value(Unexpected::Str(s.as_str()), &"u32 or null"))
}

pub fn str_to_u32<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    Ok(str_to_maybe_u32(d)?.unwrap_or(0))
}

pub fn adjust_mods_maybe<'de, D: Deserializer<'de>>(d: D) -> Result<Option<GameMods>, D::Error> {
    let s: Option<String> = Deserialize::deserialize(d)?;

    let mods = match s.as_deref() {
        None => return Ok(None),
        Some("None") => GameMods::NoMod,
        Some(s) => {
            let mut mods = GameMods::NoMod;

            for result in s.split(',').map(GameMods::from_str) {
                match result {
                    Ok(m) => mods |= m,
                    Err(why) => return Err(Error::custom(why)),
                }
            }

            mods
        }
    };

    Ok(Some(mods))
}

pub fn adjust_mods<'de, D: Deserializer<'de>>(d: D) -> Result<GameMods, D::Error> {
    Ok(adjust_mods_maybe(d)?.unwrap_or_default())
}

pub fn expect_negative_u32<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let i: i64 = Deserialize::deserialize(d)?;
    Ok(i.max(0) as u32)
}
