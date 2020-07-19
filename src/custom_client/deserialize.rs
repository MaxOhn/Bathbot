use chrono::{offset::TimeZone, DateTime, Utc};
use rosu::models::{ApprovalStatus, GameMode, GameMods, Grade};
use serde::{de, Deserialize, Deserializer};
use std::{convert::TryFrom, str::FromStr};

pub fn adjust_mode<'de, D>(d: D) -> Result<GameMode, D::Error>
where
    D: Deserializer<'de>,
{
    let m: &str = Deserialize::deserialize(d)?;
    let m = match m {
        "osu" => GameMode::STD,
        "taiko" => GameMode::TKO,
        "fruits" => GameMode::CTB,
        "mania" => GameMode::MNA,
        _ => panic!("Could not parse mode '{}'", m),
    };
    Ok(m)
}
pub fn str_to_maybe_date<'de, D>(d: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = match Deserialize::deserialize(d) {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };
    Utc.datetime_from_str(&s, "%F %T")
        .map(Some)
        .map_err(de::Error::custom)
}

pub fn str_to_date<'de, D>(d: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(str_to_maybe_date(d)?.unwrap())
}

pub fn str_to_maybe_f32<'de, D>(d: D) -> Result<Option<f32>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Deserialize::deserialize(d)?;
    Ok(s.and_then(|s| f32::from_str(&s).ok()))
}

pub fn str_to_f32<'de, D>(d: D) -> Result<f32, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(str_to_maybe_f32(d)?.unwrap_or_else(|| 0.0))
}

pub fn num_to_mode<'de, D>(d: D) -> Result<GameMode, D::Error>
where
    D: Deserializer<'de>,
{
    let num: u8 = Deserialize::deserialize(d)?;
    Ok(GameMode::from(num))
}

pub fn str_to_approved<'de, D>(d: D) -> Result<ApprovalStatus, D::Error>
where
    D: Deserializer<'de>,
{
    let num: i8 = Deserialize::deserialize(d)?;
    Ok(ApprovalStatus::from(num))
}

pub fn adjust_mods<'de, D>(d: D) -> Result<GameMods, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(d)?;
    if "None" == s.as_str() {
        return Ok(GameMods::NoMod);
    }
    let mods = s
        .split(',')
        .map(|m| {
            // TODO: Simplify this again with rosu 0.2.3
            let result = GameMods::try_from(m);
            if result.is_err() {
                let m = match m {
                    "K1" => GameMods::Key1,
                    "K2" => GameMods::Key2,
                    "K3" => GameMods::Key3,
                    "K4" => GameMods::Key4,
                    "K5" => GameMods::Key5,
                    "K6" => GameMods::Key6,
                    "K7" => GameMods::Key7,
                    "K8" => GameMods::Key8,
                    "K9" => GameMods::Key9,
                    _ => return result,
                };
                return Ok(m);
            }
            result
        })
        .collect::<Result<Vec<GameMods>, _>>()
        .map_err(de::Error::custom)?
        .into_iter()
        .fold(GameMods::NoMod, |mods, next| mods | next);
    Ok(mods)
}

pub fn str_to_grade<'de, D>(d: D) -> Result<Grade, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(d)?;
    Grade::try_from(s.as_str()).map_err(de::Error::custom)
}
