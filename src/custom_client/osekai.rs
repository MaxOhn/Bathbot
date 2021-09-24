use groups::*;

use super::deserialize::str_to_u32;

use rosu_v2::model::{GameMode, GameMods};
use serde::{
    de::{Error, MapAccess, Unexpected, Visitor},
    Deserialize, Deserializer,
};
use std::{cmp::Ordering, fmt};

#[derive(Clone, Debug, Deserialize)]
pub struct OsekaiMap {
    #[serde(rename = "Artist")]
    pub artist: String,
    #[serde(rename = "Mapper")]
    pub creator: String,
    #[serde(rename = "MapperID")]
    pub creator_id: u32,
    #[serde(rename = "BeatmapID")]
    pub map_id: u32,
    #[serde(rename = "MapsetID")]
    pub mapset_id: u32,
    #[serde(rename = "MedalName")]
    pub medal_name: String,
    #[serde(rename = "Gamemode")]
    pub mode: GameMode,
    #[serde(rename = "Difficulty")]
    pub stars: f32,
    #[serde(rename = "SongTitle")]
    pub title: String,
    #[serde(rename = "DifficultyName")]
    pub version: String,
}

#[derive(Deserialize)]
pub(super) struct OsekaiComments(pub(super) Option<Vec<OsekaiComment>>);

#[derive(Clone, Debug, Deserialize)]
pub struct OsekaiComment {
    #[serde(rename = "ID")]
    pub comment_id: u32,
    #[serde(rename = "PostText")]
    pub content: String,
    #[serde(rename = "ParentComment")]
    pub parent_id: Option<u32>,
    #[serde(rename = "UserID")]
    pub user_id: u32,
    #[serde(rename = "Username")]
    pub username: String,
    #[serde(rename = "VoteSum", deserialize_with = "str_to_u32")]
    pub vote_sum: u32,
}

pub(super) struct OsekaiMedals(pub(super) Vec<OsekaiMedal>);

impl<'de> Deserialize<'de> for OsekaiMedals {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self(d.deserialize_map(OsekaiGroupingVisitor)?))
    }
}

struct OsekaiGroupingVisitor;

impl<'de> Visitor<'de> for OsekaiGroupingVisitor {
    type Value = Vec<OsekaiMedal>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an object containing fields mapping to a list of osekai medals")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut medals = Vec::with_capacity(256);

        while let Some((_, mut medals_)) = map.next_entry::<&str, Vec<OsekaiMedal>>()? {
            medals.append(&mut medals_);
        }

        Ok(medals)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OsekaiMedal {
    #[serde(rename = "MedalID")]
    pub medal_id: u32,
    pub name: String,
    #[serde(rename = "Link")]
    pub icon_url: String,
    pub description: String,
    #[serde(deserialize_with = "osekai_mode")]
    pub restriction: Option<GameMode>,
    pub grouping: String,
    pub solution: Option<String>,
    #[serde(deserialize_with = "osekai_mods")]
    pub mods: Option<GameMods>,
    #[serde(rename = "ModeOrder")]
    pub mode_order: usize,
    pub ordering: usize,
}

pub mod groups {
    pub const SKILL: &'static str = "Skill";
    pub const DEDICATION: &'static str = "Dedication";
    pub const HUSH_HUSH: &'static str = "Hush-Hush";
    pub const BEATMAP_PACKS: &'static str = "Beatmap Packs";
    pub const BEATMAP_CHALLENGE_PACKS: &'static str = "Beatmap Challenge Packs";
    pub const SEASONAL_SPOTLIGHTS: &'static str = "Seasonal Spotlights";
    pub const BEATMAP_SPOTLIGHTS: &'static str = "Beatmap Spotlights";
    pub const MOD_INTRODUCTION: &'static str = "Mod Introduction";
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct OsekaiGrouping<'s>(pub &'s str);

impl<'s> OsekaiGrouping<'s> {
    pub fn order(&self) -> u32 {
        match self.0 {
            SKILL => 0,
            DEDICATION => 1,
            HUSH_HUSH => 2,
            BEATMAP_PACKS => 3,
            BEATMAP_CHALLENGE_PACKS => 4,
            SEASONAL_SPOTLIGHTS => 5,
            BEATMAP_SPOTLIGHTS => 6,
            MOD_INTRODUCTION => 7,
            _ => 8,
        }
    }
}

impl<'s> fmt::Display for OsekaiGrouping<'s> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl<'s> Ord for OsekaiGrouping<'s> {
    fn cmp(&self, other: &OsekaiGrouping<'s>) -> Ordering {
        self.order().cmp(&other.order())
    }
}

impl<'s> PartialOrd for OsekaiGrouping<'s> {
    fn partial_cmp(&self, other: &OsekaiGrouping<'s>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub const MEDAL_GROUPS: [OsekaiGrouping; 8] = [
    OsekaiGrouping(SKILL),
    OsekaiGrouping(DEDICATION),
    OsekaiGrouping(HUSH_HUSH),
    OsekaiGrouping(BEATMAP_PACKS),
    OsekaiGrouping(BEATMAP_CHALLENGE_PACKS),
    OsekaiGrouping(SEASONAL_SPOTLIGHTS),
    OsekaiGrouping(BEATMAP_SPOTLIGHTS),
    OsekaiGrouping(MOD_INTRODUCTION),
];

impl OsekaiMedal {
    fn grouping_order(&self) -> u32 {
        OsekaiGrouping(self.grouping.as_str()).order()
    }
}

impl PartialEq for OsekaiMedal {
    fn eq(&self, other: &Self) -> bool {
        self.medal_id == other.medal_id
    }
}

impl Eq for OsekaiMedal {}

impl PartialOrd for OsekaiMedal {
    fn partial_cmp(&self, other: &OsekaiMedal) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OsekaiMedal {
    fn cmp(&self, other: &OsekaiMedal) -> Ordering {
        self.grouping_order()
            .cmp(&other.grouping_order())
            .then_with(|| self.medal_id.cmp(&other.medal_id))
    }
}

fn osekai_mode<'de, D: Deserializer<'de>>(d: D) -> Result<Option<GameMode>, D::Error> {
    d.deserialize_option(OsekaiModeVisitor)
}

struct OsekaiModeVisitor;

impl<'de> Visitor<'de> for OsekaiModeVisitor {
    type Value = Option<GameMode>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a u8 or a string")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        let mode = match v {
            "NULL" => return Ok(None),
            "0" | "osu" | "osu!" => GameMode::STD,
            "1" | "taiko" | "tko" => GameMode::TKO,
            "2" | "ctb" | "fruits" => GameMode::CTB,
            "3" | "mania" | "mna" => GameMode::MNA,
            _ => {
                return Err(Error::invalid_value(
                    Unexpected::Str(v),
                    &r#""NULL", "0", "osu", "osu!", "1", "taiko", "tko", "2", "ctb", "fruits", "3", "mania", or "mna""#,
                ))
            }
        };

        Ok(Some(mode))
    }

    fn visit_u64<E: Error>(self, v: u64) -> Result<Self::Value, E> {
        match v {
            0 => Ok(Some(GameMode::STD)),
            1 => Ok(Some(GameMode::TKO)),
            2 => Ok(Some(GameMode::CTB)),
            3 => Ok(Some(GameMode::MNA)),
            _ => Err(Error::invalid_value(
                Unexpected::Unsigned(v),
                &"0, 1, 2, or 3",
            )),
        }
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }

    fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
}

fn osekai_mods<'de, D: Deserializer<'de>>(d: D) -> Result<Option<GameMods>, D::Error> {
    d.deserialize_option(OsekaiModsVisitor)
}

struct OsekaiModsVisitor;

impl<'de> Visitor<'de> for OsekaiModsVisitor {
    type Value = Option<GameMods>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a u8 or a string")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        let mut mods = GameMods::default();

        for mod_ in v.split(',').map(str::trim) {
            if let Ok(mod_) = mod_.parse() {
                mods |= mod_;
            } else {
                return Err(Error::invalid_value(
                    Unexpected::Str(mod_),
                    &r#"a valid mod abbreviation"#,
                ));
            }
        }

        Ok(Some(mods))
    }

    fn visit_u64<E: Error>(self, v: u64) -> Result<Self::Value, E> {
        use std::convert::TryInto;

        let bits = v.try_into().map_err(|_| {
            Error::invalid_value(
                Unexpected::Unsigned(v),
                &"a valid u32 representing a mod combination",
            )
        })?;

        Ok(GameMods::from_bits(bits))
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }

    fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
}
