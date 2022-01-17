use std::{cmp::Ordering, fmt, marker::PhantomData, str::FromStr};

use rosu_v2::{
    model::{GameMode, GameMods},
    prelude::Username,
};
use serde::{
    de::{Error, MapAccess, Unexpected, Visitor},
    Deserialize, Deserializer,
};

use crate::{
    embeds::RankingKindData,
    util::{
        constants::common_literals::{COUNTRY, CTB, FRUITS, MANIA, OSU, RANK, TAIKO},
        CountryCode,
    },
};

use self::groups::*;

use super::deserialize::{str_to_f32, str_to_u32};

pub trait OsekaiRanking {
    const FORM: &'static str;
    const REQUEST: &'static str;
    const RANKING: RankingKindData;
    type Entry: for<'de> Deserialize<'de>;
}

pub struct Rarity;
pub struct MedalCount;
pub struct Replays;
pub struct TotalPp;
pub struct StandardDeviation;
pub struct Badges;
pub struct RankedMapsets;
pub struct LovedMapsets;
pub struct Subscribers;

impl OsekaiRanking for Rarity {
    const FORM: &'static str = "Rarity";
    const REQUEST: &'static str = "osekai rarity";
    const RANKING: RankingKindData = RankingKindData::OsekaiRarity;
    type Entry = OsekaiRarityEntry;
}

impl OsekaiRanking for MedalCount {
    const FORM: &'static str = "Users";
    const REQUEST: &'static str = "osekai users";
    const RANKING: RankingKindData = RankingKindData::OsekaiMedalCount;
    type Entry = OsekaiUserEntry;
}

impl OsekaiRanking for Replays {
    const FORM: &'static str = "Replays";
    const REQUEST: &'static str = "osekai replays";
    const RANKING: RankingKindData = RankingKindData::OsekaiReplays;
    type Entry = OsekaiRankingEntry<usize>;
}

impl OsekaiRanking for TotalPp {
    const FORM: &'static str = "Total pp";
    const REQUEST: &'static str = "osekai total pp";
    const RANKING: RankingKindData = RankingKindData::OsekaiTotalPp;
    type Entry = OsekaiRankingEntry<u32>;
}

impl OsekaiRanking for StandardDeviation {
    const FORM: &'static str = "Standard Deviation";
    const REQUEST: &'static str = "osekai standard deviation";
    const RANKING: RankingKindData = RankingKindData::OsekaiStandardDeviation;
    type Entry = OsekaiRankingEntry<u32>;
}

impl OsekaiRanking for Badges {
    const FORM: &'static str = "Badges";
    const REQUEST: &'static str = "osekai badges";
    const RANKING: RankingKindData = RankingKindData::OsekaiBadges;
    type Entry = OsekaiRankingEntry<usize>;
}

impl OsekaiRanking for RankedMapsets {
    const FORM: &'static str = "Ranked Mapsets";
    const REQUEST: &'static str = "osekai ranked mapsets";
    const RANKING: RankingKindData = RankingKindData::OsekaiRankedMapsets;
    type Entry = OsekaiRankingEntry<usize>;
}

impl OsekaiRanking for LovedMapsets {
    const FORM: &'static str = "Loved Mapsets";
    const REQUEST: &'static str = "osekai loved mapsets";
    const RANKING: RankingKindData = RankingKindData::OsekaiLovedMapsets;
    type Entry = OsekaiRankingEntry<usize>;
}

impl OsekaiRanking for Subscribers {
    const FORM: &'static str = "Subscribers";
    const REQUEST: &'static str = "osekai subscribers";
    const RANKING: RankingKindData = RankingKindData::OsekaiSubscribers;
    type Entry = OsekaiRankingEntry<usize>;
}

#[derive(Deserialize)]
pub(super) struct OsekaiMaps(pub(super) Option<Vec<OsekaiMap>>);

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
    #[serde(rename = "VoteSum", deserialize_with = "str_to_u32")]
    pub vote_sum: u32,
}

#[derive(Deserialize)]
pub(super) struct OsekaiComments(pub(super) Option<Vec<OsekaiComment>>);

#[derive(Clone, Debug, Deserialize)]
pub struct OsekaiComment {
    #[serde(rename = "ID")]
    pub comment_id: u32,
    #[serde(rename = "PostText")]
    pub content: String,
    #[serde(rename = "Parent")]
    pub parent_id: u32,
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

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    pub const SKILL: &str = "Skill";
    pub const DEDICATION: &str = "Dedication";
    pub const HUSH_HUSH: &str = "Hush-Hush";
    pub const BEATMAP_PACKS: &str = "Beatmap Packs";
    pub const BEATMAP_CHALLENGE_PACKS: &str = "Beatmap Challenge Packs";
    pub const SEASONAL_SPOTLIGHTS: &str = "Seasonal Spotlights";
    pub const BEATMAP_SPOTLIGHTS: &str = "Beatmap Spotlights";
    pub const MOD_INTRODUCTION: &str = "Mod Introduction";
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

pub const MEDAL_GROUPS: [OsekaiGrouping<'_>; 8] = [
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

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a u8 or a string")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        let mode = match v {
            "NULL" => return Ok(None),
            "0" | OSU | "osu!" => GameMode::STD,
            "1" | TAIKO | "tko" => GameMode::TKO,
            "2" | "catch" | CTB | FRUITS => GameMode::CTB,
            "3" | MANIA | "mna" => GameMode::MNA,
            _ => {
                return Err(Error::invalid_value(
                    Unexpected::Str(v),
                    &r#""NULL", "0", "osu", "osu!", "1", "taiko", "tko", "2", "catch", "ctb", "fruits", "3", "mania", or "mna""#,
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

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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

#[derive(Debug)]
pub struct OsekaiRankingEntry<T> {
    pub country: String,
    pub country_code: CountryCode,
    pub rank: u32,
    pub user_id: u32,
    pub username: Username,
    value: ValueWrapper<T>,
}

impl<T: Copy> OsekaiRankingEntry<T> {
    pub fn value(&self) -> T {
        self.value.0
    }
}

#[derive(Copy, Clone)]
struct ValueWrapper<T>(T);

impl<T: fmt::Debug> fmt::Debug for ValueWrapper<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl<T: fmt::Display> fmt::Display for ValueWrapper<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'de, T: Deserialize<'de> + FromStr> Deserialize<'de> for ValueWrapper<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s: &str = Deserialize::deserialize(d)?;

        let value = s
            .parse()
            .map_err(|_| Error::custom(format!("failed to parse `{}` into ranking value", s)))?;

        Ok(Self(value))
    }
}

impl<'de, T: Deserialize<'de> + FromStr> Deserialize<'de> for OsekaiRankingEntry<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_map(OsekaiRankingEntryVisitor::new())
    }
}

struct OsekaiRankingEntryVisitor<T> {
    data: PhantomData<T>,
}

impl<T> OsekaiRankingEntryVisitor<T> {
    fn new() -> Self {
        Self { data: PhantomData }
    }
}

impl<'de, T: Deserialize<'de> + FromStr> Visitor<'de> for OsekaiRankingEntryVisitor<T> {
    type Value = OsekaiRankingEntry<T>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an osekai ranking entry")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut rank: Option<&str> = None;
        let mut country_code = None;
        let mut country = None;
        let mut username = None;
        let mut user_id: Option<&str> = None;
        let mut value = None;

        while let Some(key) = map.next_key()? {
            match key {
                RANK => rank = Some(map.next_value()?),
                "countrycode" => country_code = Some(map.next_value()?),
                COUNTRY => country = Some(map.next_value()?),
                "username" => username = Some(map.next_value()?),
                "userid" => user_id = Some(map.next_value()?),
                _ => value = Some(map.next_value()?),
            }
        }

        let rank: &str = rank.ok_or_else(|| Error::missing_field(RANK))?;
        let rank = rank.parse().map_err(|_| {
            Error::invalid_value(Unexpected::Str(rank), &"a string containing a u32")
        })?;

        let country_code = country_code.ok_or_else(|| Error::missing_field("countrycode"))?;
        let country = country.ok_or_else(|| Error::missing_field(COUNTRY))?;
        let username = username.ok_or_else(|| Error::missing_field("username"))?;

        let user_id: &str = user_id.ok_or_else(|| Error::missing_field("userid"))?;
        let user_id = user_id.parse().map_err(|_| {
            Error::invalid_value(Unexpected::Str(user_id), &"a string containing a u32")
        })?;

        let value = value.ok_or_else(|| Error::custom("missing field for ranking value"))?;

        Ok(Self::Value {
            rank,
            country_code,
            country,
            username,
            user_id,
            value,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct OsekaiUserEntry {
    #[serde(deserialize_with = "str_to_u32")]
    pub rank: u32,
    #[serde(rename = "countrycode")]
    pub country_code: CountryCode,
    pub country: String,
    pub username: Username,
    #[serde(rename = "medalCount", deserialize_with = "str_to_u32")]
    pub medal_count: u32,
    #[serde(rename = "rarestmedal")]
    pub rarest_medal: String,
    #[serde(rename = "link")]
    pub rarest_icon_url: String,
    #[serde(rename = "userid", deserialize_with = "str_to_u32")]
    pub user_id: u32,
    #[serde(deserialize_with = "str_to_f32")]
    pub completion: f32,
}

#[derive(Debug, Deserialize)]
pub struct OsekaiRarityEntry {
    #[serde(deserialize_with = "str_to_u32")]
    pub rank: u32,
    #[serde(rename = "link")]
    pub icon_url: String,
    #[serde(rename = "medalname")]
    pub medal_name: String,
    #[serde(rename = "medalid", deserialize_with = "str_to_u32")]
    pub medal_id: u32,
    pub description: String,
    #[serde(rename = "possessionRate", deserialize_with = "str_to_f32")]
    pub possession_percent: f32,
    #[serde(rename = "gameMode", deserialize_with = "osekai_mode")]
    pub mode: Option<GameMode>,
}
