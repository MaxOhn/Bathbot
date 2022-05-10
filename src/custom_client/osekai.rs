use std::{cmp::Ordering, fmt, marker::PhantomData, str::FromStr};

use chrono::{Date, Utc};
use rkyv::{with::ArchiveWith, Archive, Deserialize as RkyvDeserialize, Serialize};
use rosu_v2::{
    model::{GameMode, GameMods},
    prelude::Username,
};
use serde::{
    de::{Error, MapAccess, Unexpected, Visitor},
    Deserialize, Deserializer,
};
use twilight_interactions::command::{CommandOption, CreateOption};

use crate::{embeds::RankingKindData, util::CountryCode};

// use self::groups::*;

use super::{rkyv_impls::UsernameWrapper, str_to_date, str_to_f32, str_to_u32, DateWrapper};

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

#[derive(Archive, Clone, Debug, Deserialize, RkyvDeserialize, Serialize)]
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
    pub grouping: MedalGroup,
    pub solution: Option<String>,
    #[serde(deserialize_with = "osekai_mods")]
    pub mods: Option<GameMods>,
    #[serde(rename = "ModeOrder")]
    pub mode_order: usize,
    pub ordering: usize,
}

pub static MEDAL_GROUPS: [MedalGroup; 8] = [
    MedalGroup::Skill,
    MedalGroup::Dedication,
    MedalGroup::HushHush,
    MedalGroup::BeatmapPacks,
    MedalGroup::BeatmapChallengePacks,
    MedalGroup::SeasonalSpotlights,
    MedalGroup::BeatmapSpotlights,
    MedalGroup::ModIntroduction,
];

#[derive(
    Archive,
    Copy,
    Clone,
    CommandOption,
    CreateOption,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    RkyvDeserialize,
    Serialize,
)]
pub enum MedalGroup {
    #[option(name = "Skill", value = "skill")]
    Skill,
    #[option(name = "Dedication", value = "dedication")]
    Dedication,
    #[option(name = "Hush-Hush", value = "hush_hush")]
    HushHush,
    #[option(name = "Beatmap Packs", value = "map_packs")]
    BeatmapPacks,
    #[option(name = "Beatmap Challenge Packs", value = "map_challenge_packs")]
    BeatmapChallengePacks,
    #[option(name = "Seasonal Spotlights", value = "seasonal_spotlights")]
    SeasonalSpotlights,
    #[option(name = "Beatmap Spotlights", value = "map_spotlights")]
    BeatmapSpotlights,
    #[option(name = "Mod Introduction", value = "mod_intro")]
    ModIntroduction,
}

impl MedalGroup {
    pub fn order(self) -> u32 {
        self as u32
    }

    pub fn as_str(self) -> &'static str {
        match self {
            MedalGroup::Skill => "Skill",
            MedalGroup::Dedication => "Dedication",
            MedalGroup::HushHush => "Hush-Hush",
            MedalGroup::BeatmapPacks => "Beatmap Packs",
            MedalGroup::BeatmapChallengePacks => "Beatmap Challenge Packs",
            MedalGroup::SeasonalSpotlights => "Seasonal Spotlights",
            MedalGroup::BeatmapSpotlights => "Beatmap Spotlights",
            MedalGroup::ModIntroduction => "Mod Introduction",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        let group = match s {
            "Skill" => MedalGroup::Skill,
            "Dedication" => MedalGroup::Dedication,
            "Hush-Hush" => MedalGroup::HushHush,
            "Beatmap Packs" => MedalGroup::BeatmapPacks,
            "Beatmap Challenge Packs" => MedalGroup::BeatmapChallengePacks,
            "Seasonal Spotlights" => MedalGroup::SeasonalSpotlights,
            "Beatmap Spotlights" => MedalGroup::BeatmapSpotlights,
            "Mod Introduction" => MedalGroup::ModIntroduction,
            _ => return None,
        };

        Some(group)
    }
}

impl fmt::Display for MedalGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

struct MedalGroupVisitor;

impl<'de> Visitor<'de> for MedalGroupVisitor {
    type Value = MedalGroup;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a valid medal group")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let group = match v {
            0 => MedalGroup::Skill,
            1 => MedalGroup::Dedication,
            2 => MedalGroup::HushHush,
            3 => MedalGroup::BeatmapPacks,
            4 => MedalGroup::BeatmapChallengePacks,
            5 => MedalGroup::SeasonalSpotlights,
            6 => MedalGroup::BeatmapSpotlights,
            7 => MedalGroup::ModIntroduction,
            _ => return Err(Error::invalid_type(Unexpected::Unsigned(v), &self)),
        };

        Ok(group)
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        MedalGroup::from_str(v).ok_or_else(|| Error::invalid_type(Unexpected::Str(v), &self))
    }
}

impl<'de> Deserialize<'de> for MedalGroup {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s: &str = Deserialize::deserialize(d)?;

        Self::from_str(s)
            .ok_or_else(|| Error::invalid_value(Unexpected::Str(s), &"a valid medal group"))
    }
}

impl OsekaiMedal {
    fn grouping_order(&self) -> u32 {
        self.grouping.order()
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
            "0" | "osu" | "osu!" => GameMode::STD,
            "1" | "taiko" | "tko" => GameMode::TKO,
            "2" | "catch" | "ctb" | "fruits" => GameMode::CTB,
            "3" | "mania" | "mna" => GameMode::MNA,
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

#[derive(Archive, Debug, RkyvDeserialize, Serialize)]
#[archive(as = "ArchivedOsekaiRankingEntry<T>")]
pub struct OsekaiRankingEntry<T: Archive> {
    pub country: String,
    pub country_code: CountryCode,
    pub rank: u32,
    pub user_id: u32,
    #[with(UsernameWrapper)]
    pub username: Username,
    value: ValueWrapper<T>,
}

pub struct ArchivedOsekaiRankingEntry<T: Archive> {
    pub country: <String as Archive>::Archived,
    pub country_code: <CountryCode as Archive>::Archived,
    pub rank: u32,
    pub user_id: u32,
    pub username: <UsernameWrapper as ArchiveWith<Username>>::Archived,
    value: <ValueWrapper<T> as Archive>::Archived,
}

impl<T: Copy + Archive> OsekaiRankingEntry<T> {
    pub fn value(&self) -> T {
        self.value.0
    }
}

impl<T> ArchivedOsekaiRankingEntry<T>
where
    T: Archive,
    <T as Archive>::Archived: Copy,
{
    pub fn value(&self) -> T::Archived {
        self.value.0
    }
}

#[derive(Archive, Copy, Clone, RkyvDeserialize, Serialize)]
#[archive(as = "ValueWrapper<T::Archived>")]
pub struct ValueWrapper<T>(T);

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
            .map_err(|_| Error::custom(format!("failed to parse `{s}` into ranking value")))?;

        Ok(Self(value))
    }
}

impl<'de, T: Deserialize<'de> + FromStr + Archive> Deserialize<'de> for OsekaiRankingEntry<T> {
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

impl<'de, T: Deserialize<'de> + FromStr + Archive> Visitor<'de> for OsekaiRankingEntryVisitor<T> {
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
                "rank" => rank = Some(map.next_value()?),
                "countrycode" => country_code = Some(map.next_value()?),
                "country" => country = Some(map.next_value()?),
                "username" => username = Some(map.next_value()?),
                "userid" => user_id = Some(map.next_value()?),
                _ => value = Some(map.next_value()?),
            }
        }

        let rank: &str = rank.ok_or_else(|| Error::missing_field("rank"))?;
        let rank = rank.parse().map_err(|_| {
            Error::invalid_value(Unexpected::Str(rank), &"a string containing a u32")
        })?;

        let country_code = country_code.ok_or_else(|| Error::missing_field("countrycode"))?;
        let country = country.ok_or_else(|| Error::missing_field("country"))?;
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

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, Serialize)]
pub struct OsekaiUserEntry {
    #[serde(deserialize_with = "str_to_u32")]
    pub rank: u32,
    #[serde(rename = "countrycode")]
    pub country_code: CountryCode,
    pub country: String,
    #[with(UsernameWrapper)]
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

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, Serialize)]
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

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, Serialize)]
pub struct OsekaiBadge {
    #[serde(deserialize_with = "str_to_date")]
    #[with(DateWrapper)]
    pub awarded_at: Date<Utc>,
    pub description: String,
    #[serde(rename = "id", deserialize_with = "str_to_u32")]
    pub badge_id: u32,
    pub image_url: String,
    pub name: String,
    #[serde(deserialize_with = "string_of_vec_of_u32s")]
    pub users: Vec<u32>,
}

fn string_of_vec_of_u32s<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u32>, D::Error> {
    let stringified_vec: &str = Deserialize::deserialize(d)?;

    stringified_vec[1..stringified_vec.len() - 1]
        .split(',')
        .map(|s| s.parse().map_err(|_| s))
        .collect::<Result<Vec<u32>, _>>()
        .map_err(|s| Error::invalid_value(Unexpected::Str(s), &"u32"))
}

// data contains many more fields but none of use as of now
#[derive(Debug, Deserialize)]
pub struct OsekaiBadgeOwner {
    pub avatar_url: String,
    pub country_code: CountryCode,
    #[serde(rename = "id")]
    pub user_id: u32,
    #[serde(rename = "name")]
    pub username: Username,
}
