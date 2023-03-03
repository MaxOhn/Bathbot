use std::{
    cmp::Ordering,
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    marker::PhantomData,
    str::FromStr,
};

use rkyv::{
    with::{ArchiveWith, Raw},
    Archive, Deserialize as RkyvDeserialize, Serialize,
};
use rosu_v2::{
    model::{GameMode, GameMods},
    prelude::Username,
};
use serde::{
    de::{Error, IgnoredAny, MapAccess, SeqAccess, Unexpected, Visitor},
    Deserialize, Deserializer,
};
use time::Date;
use twilight_interactions::command::{CommandOption, CreateOption};

use crate::{
    rkyv_impls::{DateWrapper, UsernameWrapper},
    CountryCode, RankingKind,
};

use super::deser;

pub trait OsekaiRanking {
    const FORM: &'static str;
    const RANKING: RankingKind;

    type Deser: for<'de> Deserialize<'de>;
    type Entry: From<Self::Deser>;
}

macro_rules! define_ranking {
    ($struct:ident, $form:literal, $ranking:ident, $deser:ident, $entry:ident $( <$ty:ty>, $field:literal )? ) => {
        pub struct $struct;

        impl OsekaiRanking for $struct {
            const FORM: &'static str = $form;
            const RANKING: RankingKind = RankingKind::$ranking;

            type Deser = $deser;
            type Entry = $entry $( <$ty> )?;
        }

        $(
            pub struct $deser {
                inner: OsekaiRankingEntry<$ty>,
            }

            impl From<$deser> for OsekaiRankingEntry<$ty> {
                #[inline]
                fn from(entry: $deser) -> Self {
                    entry.inner
                }
            }

            impl<'de> Deserialize<'de> for $deser {
                #[inline]
                fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                    d.deserialize_map(OsekaiRankingEntryVisitor::new($field))
                        .map(|inner| Self { inner })
                }
            }
        )?
    };
}

define_ranking! {
    Rarity,
    "Rarity",
    OsekaiRarity,
    OsekaiRarityEntry,
    OsekaiRarityEntry
}

define_ranking! {
    MedalCount,
    "Users",
    OsekaiMedalCount,
    OsekaiUserEntry,
    OsekaiUserEntry
}

define_ranking! {
    Replays,
    "Replays",
    OsekaiReplays,
    OsekaiRankingReplaysEntry,
    OsekaiRankingEntry<usize>,
    "replays"
}

define_ranking! {
    TotalPp,
    "Total pp",
    OsekaiTotalPp,
    OsekaiRankingTotalPpEntry,
    OsekaiRankingEntry<u32>,
    "tpp"
}

define_ranking! {
    StandardDeviation,
    "Standard Deviation",
    OsekaiStandardDeviation,
    OsekaiRankingStandardDeviationEntry,
    OsekaiRankingEntry<u32>,
    "spp"
}

define_ranking! {
    Badges,
    "Badges",
    OsekaiBadges,
    OsekaiRankingBadgesEntry,
    OsekaiRankingEntry<usize>,
    "badges"
}

define_ranking! {
    RankedMapsets,
    "Ranked Mapsets",
    OsekaiRankedMapsets,
    OsekaiRankingRankedMapsetsEntry,
    OsekaiRankingEntry<usize>,
    "ranked"
}

define_ranking! {
    LovedMapsets,
    "Loved Mapsets",
    OsekaiLovedMapsets,
    OsekaiRankingLovedMapsetsEntry,
    OsekaiRankingEntry<usize>,
    "loved"
}

define_ranking! {
    Subscribers,
    "Subscribers",
    OsekaiSubscribers,
    OsekaiRankingSubscribersEntry,
    OsekaiRankingEntry<usize>,
    "subscribers"
}

#[derive(Deserialize)]
pub struct OsekaiMaps(pub Option<Vec<OsekaiMap>>);

#[derive(Clone, Debug, Deserialize)]
pub struct OsekaiMap {
    #[serde(rename = "Artist")]
    pub artist: Box<str>,
    #[serde(rename = "Mapper")]
    pub creator: Username,
    #[serde(rename = "MapperID")]
    pub creator_id: u32,
    #[serde(rename = "BeatmapID")]
    pub map_id: u32,
    #[serde(rename = "MapsetID")]
    pub mapset_id: u32,
    #[serde(rename = "MedalName")]
    pub medal_name: Box<str>,
    #[serde(rename = "Gamemode")]
    pub mode: GameMode,
    #[serde(rename = "Difficulty")]
    pub stars: f32,
    #[serde(rename = "SongTitle")]
    pub title: Box<str>,
    #[serde(rename = "DifficultyName")]
    pub version: Box<str>,
    #[serde(rename = "VoteSum", with = "deser::u32_string")]
    pub vote_sum: u32,
}

#[derive(Deserialize)]
pub struct OsekaiComments(pub Option<Vec<OsekaiComment>>);

#[derive(Clone, Debug, Deserialize)]
pub struct OsekaiComment {
    #[serde(rename = "ID")]
    pub comment_id: u32,
    #[serde(rename = "PostText")]
    pub content: Box<str>,
    #[serde(rename = "Parent")]
    pub parent_id: u32,
    #[serde(rename = "UserID")]
    pub user_id: u32,
    #[serde(rename = "Username")]
    pub username: Username,
    #[serde(rename = "VoteSum", with = "deser::u32_string")]
    pub vote_sum: u32,
}

pub struct OsekaiMedals(pub Vec<OsekaiMedal>);

impl<'de> Deserialize<'de> for OsekaiMedals {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct OsekaiGroupingVisitor;

        impl<'de> Visitor<'de> for OsekaiGroupingVisitor {
            type Value = Vec<OsekaiMedal>;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("an object containing fields mapping to a list of osekai medals")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut medals = Vec::with_capacity(256);

                while let Some((_, mut medals_)) = map.next_entry::<&str, Vec<OsekaiMedal>>()? {
                    medals.append(&mut medals_);
                }

                Ok(medals)
            }
        }

        Ok(Self(d.deserialize_map(OsekaiGroupingVisitor)?))
    }
}

#[derive(Archive, Clone, Debug, Deserialize, RkyvDeserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct OsekaiMedal {
    #[serde(rename = "MedalID")]
    pub medal_id: u32,
    pub name: Box<str>,
    #[serde(rename = "Link")]
    pub icon_url: Box<str>,
    pub description: Box<str>,
    #[serde(deserialize_with = "osekai_mode")]
    pub restriction: Option<GameMode>,
    pub grouping: MedalGroup,
    pub solution: Option<Box<str>>,
    #[serde(deserialize_with = "osekai_mods")]
    pub mods: Option<GameMods>,
    #[serde(rename = "ModeOrder")]
    pub mode_order: usize,
    pub ordering: usize,
    #[serde(rename = "Rarity")]
    pub rarity: f32,
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
impl FromStr for MedalGroup {
    type Err = ();

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let group = match s {
            "Skill" => MedalGroup::Skill,
            "Dedication" => MedalGroup::Dedication,
            "Hush-Hush" => MedalGroup::HushHush,
            "Beatmap Packs" => MedalGroup::BeatmapPacks,
            "Beatmap Challenge Packs" => MedalGroup::BeatmapChallengePacks,
            "Seasonal Spotlights" => MedalGroup::SeasonalSpotlights,
            "Beatmap Spotlights" => MedalGroup::BeatmapSpotlights,
            "Mod Introduction" => MedalGroup::ModIntroduction,
            _ => return Err(()),
        };

        Ok(group)
    }
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
}

impl Display for MedalGroup {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for MedalGroup {
    #[inline]
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s: &str = Deserialize::deserialize(d)?;

        Self::from_str(s)
            .map_err(|_| Error::invalid_value(Unexpected::Str(s), &"a valid medal group"))
    }
}

impl OsekaiMedal {
    fn grouping_order(&self) -> u32 {
        self.grouping.order()
    }
}

impl PartialEq for OsekaiMedal {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.medal_id == other.medal_id
    }
}

impl Eq for OsekaiMedal {}

impl PartialOrd for OsekaiMedal {
    #[inline]
    fn partial_cmp(&self, other: &OsekaiMedal) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OsekaiMedal {
    #[inline]
    fn cmp(&self, other: &OsekaiMedal) -> Ordering {
        self.grouping_order()
            .cmp(&other.grouping_order())
            .then_with(|| self.medal_id.cmp(&other.medal_id))
    }
}

fn osekai_mode<'de, D: Deserializer<'de>>(d: D) -> Result<Option<GameMode>, D::Error> {
    struct OsekaiModeVisitor;

    impl<'de> Visitor<'de> for OsekaiModeVisitor {
        type Value = Option<GameMode>;

        #[inline]
        fn expecting(&self, formatter: &mut Formatter<'_>) -> FmtResult {
            formatter.write_str("a u8 or a string")
        }

        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            let mode = match v {
                "NULL" => return Ok(None),
                "0" | "osu" | "osu!" => GameMode::Osu,
                "1" | "taiko" | "tko" => GameMode::Taiko,
                "2" | "catch" | "ctb" | "fruits" => GameMode::Catch,
                "3" | "mania" | "mna" => GameMode::Mania,
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
                0 => Ok(Some(GameMode::Osu)),
                1 => Ok(Some(GameMode::Taiko)),
                2 => Ok(Some(GameMode::Catch)),
                3 => Ok(Some(GameMode::Mania)),
                _ => Err(Error::invalid_value(
                    Unexpected::Unsigned(v),
                    &"0, 1, 2, or 3",
                )),
            }
        }

        #[inline]
        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            d.deserialize_any(self)
        }

        #[inline]
        fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
            self.visit_unit()
        }

        #[inline]
        fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }

    d.deserialize_option(OsekaiModeVisitor)
}

fn osekai_mods<'de, D: Deserializer<'de>>(d: D) -> Result<Option<GameMods>, D::Error> {
    struct OsekaiModsVisitor;

    impl<'de> Visitor<'de> for OsekaiModsVisitor {
        type Value = Option<GameMods>;

        fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
            f.write_str("a u8 or a string")
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

        #[inline]
        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            d.deserialize_any(self)
        }

        #[inline]
        fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
            self.visit_unit()
        }

        #[inline]
        fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }

    d.deserialize_option(OsekaiModsVisitor)
}

#[derive(Archive, Debug, RkyvDeserialize, Serialize)]
#[archive(as = "ArchivedOsekaiRankingEntry<T>")]
pub struct OsekaiRankingEntry<T: Archive> {
    pub country: Box<str>,
    pub country_code: CountryCode,
    pub rank: u32,
    pub user_id: u32,
    #[with(UsernameWrapper)]
    pub username: Username,
    value: ValueWrapper<T>,
}

pub struct ArchivedOsekaiRankingEntry<T: Archive> {
    pub country: <Box<str> as Archive>::Archived,
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

impl<T: Debug> Debug for ValueWrapper<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{:?}", self.0)
    }
}

impl<T: Display> Display for ValueWrapper<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
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

struct OsekaiRankingEntryVisitor<T> {
    api_field: &'static str,
    phantom: PhantomData<T>,
}

impl<T> OsekaiRankingEntryVisitor<T> {
    fn new(api_field: &'static str) -> Self {
        Self {
            api_field,
            phantom: PhantomData,
        }
    }
}

impl<'de, T> Visitor<'de> for OsekaiRankingEntryVisitor<T>
where
    T: Deserialize<'de> + FromStr + Archive,
{
    type Value = OsekaiRankingEntry<T>;

    fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
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
                _ if key == self.api_field => value = Some(map.next_value()?),
                _ => {
                    let _ = map.next_value::<IgnoredAny>()?;
                }
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

        let value = value.ok_or_else(|| Error::missing_field(self.api_field))?;

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

pub struct OsekaiRankingEntries<R: OsekaiRanking> {
    inner: Vec<R::Entry>,
}

impl<'de, R: OsekaiRanking> Deserialize<'de> for OsekaiRankingEntries<R> {
    #[inline]
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct OsekaiRankingEntriesVisitor<R> {
            phantom: PhantomData<R>,
        }

        impl<'de, R: OsekaiRanking> Visitor<'de> for OsekaiRankingEntriesVisitor<R> {
            type Value = OsekaiRankingEntries<R>;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("a list of osekai ranking entries")
            }

            #[inline]
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::with_capacity(seq.size_hint().unwrap_or(4));

                while let Some(elem) = seq.next_element::<R::Deser>()? {
                    entries.push(elem.into());
                }

                Ok(OsekaiRankingEntries { inner: entries })
            }
        }

        let visitor = OsekaiRankingEntriesVisitor {
            phantom: PhantomData,
        };

        d.deserialize_seq(visitor)
    }
}

impl<R: OsekaiRanking> From<OsekaiRankingEntries<R>> for Vec<R::Entry> {
    #[inline]
    fn from(entries: OsekaiRankingEntries<R>) -> Self {
        entries.inner
    }
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, Serialize)]
pub struct OsekaiUserEntry {
    #[serde(with = "deser::u32_string")]
    pub rank: u32,
    #[serde(rename = "countrycode")]
    pub country_code: CountryCode,
    pub country: Box<str>,
    #[with(UsernameWrapper)]
    pub username: Username,
    #[serde(rename = "medalCount", with = "deser::u32_string")]
    pub medal_count: u32,
    #[serde(rename = "rarestmedal")]
    pub rarest_medal: Box<str>,
    #[serde(rename = "link")]
    pub rarest_icon_url: Box<str>,
    #[serde(rename = "userid", with = "deser::u32_string")]
    pub user_id: u32,
    #[serde(with = "deser::f32_string")]
    pub completion: f32,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, Serialize)]
pub struct OsekaiRarityEntry {
    #[serde(with = "deser::u32_string")]
    pub rank: u32,
    #[serde(rename = "link")]
    pub icon_url: Box<str>,
    #[serde(rename = "medalname")]
    pub medal_name: Box<str>,
    #[serde(rename = "medalid", with = "deser::u32_string")]
    pub medal_id: u32,
    pub description: Box<str>,
    #[serde(rename = "possessionRate", with = "deser::f32_string")]
    pub possession_percent: f32,
    #[serde(rename = "gameMode", deserialize_with = "osekai_mode")]
    pub mode: Option<GameMode>,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, Serialize)]
pub struct OsekaiBadge {
    #[serde(with = "deser::date")]
    #[with(DateWrapper)]
    pub awarded_at: Date,
    pub description: Box<str>,
    #[serde(rename = "id", with = "deser::u32_string")]
    pub badge_id: u32,
    pub image_url: Box<str>,
    pub name: Box<str>,
    #[serde(deserialize_with = "string_of_vec_of_u32s")]
    #[with(Raw)]
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
    pub avatar_url: Box<str>,
    pub country_code: CountryCode,
    #[serde(rename = "id")]
    pub user_id: u32,
    #[serde(rename = "name")]
    pub username: Username,
}
