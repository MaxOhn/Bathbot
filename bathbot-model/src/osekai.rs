use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    marker::PhantomData,
    str::FromStr,
};

use eyre::{Result, WrapErr};
use form_urlencoded::Serializer as FormSerializer;
use rkyv::{
    Archive, Archived, Deserialize as RkyvDeserialize, Portable, Serialize,
    bytecheck::CheckBytes,
    niche::niching::{NaN, Null},
    string::ArchivedString,
    with::NicheInto,
};
use rosu_v2::{
    model::GameMode,
    prelude::{CountryCode, Username},
};
use serde::{
    Deserialize, Deserializer, Serialize as _,
    de::{Error, IgnoredAny, MapAccess, SeqAccess, Unexpected, Visitor},
};
use serde_urlencoded::Serializer as UrlSerializer;
use time::Date;
use twilight_interactions::command::{CommandOption, CreateOption};

use super::deser;
use crate::{
    RankingKind,
    rkyv_util::{DerefAsString, time::DateRkyv},
    rosu_v2::mode::GameModeNiche,
};

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

#[derive(Clone, Debug, Deserialize)]
pub struct OsekaiMap {
    #[serde(rename = "Song_Artist")]
    pub artist: Box<str>,
    #[serde(rename = "Mapper_Name")]
    pub creator: Username,
    #[serde(rename = "Mapper_ID")]
    pub creator_id: u32,
    #[serde(rename = "Beatmap_ID")]
    pub map_id: u32,
    #[serde(rename = "Beatmapset_ID")]
    pub mapset_id: u32,
    #[serde(rename = "Gamemode", deserialize_with = "osekai_mode")]
    pub mode: GameMode,
    #[serde(rename = "Difficulty_Rating")]
    pub stars: f32,
    #[serde(rename = "Song_Title")]
    pub title: Box<str>,
    #[serde(rename = "Difficulty_Name")]
    pub version: Box<str>,
    #[serde(rename = "VoteCount")]
    pub vote_count: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OsekaiComment {
    #[serde(rename = "ID")]
    pub comment_id: u32,
    #[serde(rename = "Text")]
    pub content: Box<str>,
    // By default osekai only sends top-level comments so parent_id is always
    // null
    // #[serde(rename = "Parent_Comment_ID")]
    // pub parent_id: Option<u32>,
    #[serde(rename = "User_ID")]
    pub user_id: u32,
    #[serde(rename = "Username")]
    pub username: Option<Username>,
    #[serde(rename = "VoteCount")]
    pub vote_count: u32,
}

#[derive(Archive, Clone, Debug, Deserialize, RkyvDeserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct OsekaiMedal {
    #[serde(rename = "Medal_ID", with = "deser::u32_string")]
    pub medal_id: u32,
    #[serde(with = "deser::u32_string")]
    pub ordering: u32,
    #[serde(rename = "Frequency", with = "deser::option_f32_string")]
    #[rkyv(with = NicheInto<NaN>)]
    pub rarity: Option<f32>,
    #[rkyv(with = DerefAsString)]
    pub name: Box<str>,
    #[serde(rename = "Link")]
    #[rkyv(with = DerefAsString)]
    icon_url_suffix: Box<str>,
    #[rkyv(with = DerefAsString)]
    pub description: Box<str>,
    #[serde(rename = "Gamemode", deserialize_with = "maybe_osekai_mode")]
    #[rkyv(with = NicheInto<GameModeNiche>)]
    pub mode: Option<GameMode>,
    pub grouping: MedalGroup,
    #[rkyv(with = NicheInto<Null>)]
    solution: Option<Box<str>>,
    #[serde(deserialize_with = "medal_mods")]
    #[rkyv(with = NicheInto<Null>)]
    pub mods: Option<Box<str>>,
    #[serde(rename = "Supports_Lazer", deserialize_with = "stringified_bool_int")]
    pub supports_lazer: bool,
    #[serde(rename = "Supports_Stable", deserialize_with = "stringified_bool_int")]
    pub supports_stable: bool,
}

pub fn stringified_bool_int<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    match <&str as Deserialize>::deserialize(d)? {
        "0" => Ok(false),
        "1" => Ok(true),
        other => Err(Error::invalid_value(
            Unexpected::Str(other),
            &r#""0" or "1""#,
        )),
    }
}

pub static MEDAL_GROUPS: [MedalGroup; 8] = [
    MedalGroup::SkillDedication,
    MedalGroup::HushHush,
    MedalGroup::HushHushExpert,
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
    Portable,
    CheckBytes,
)]
#[rkyv(as = Self)]
#[bytecheck(crate = rkyv::bytecheck)]
#[repr(u8)]
pub enum MedalGroup {
    #[option(name = "Skill & Dedication", value = "skill_dedication")]
    SkillDedication,
    #[option(name = "Hush-Hush", value = "hush_hush")]
    HushHush,
    #[option(name = "Hush-Hush (Expert)", value = "hush_hush_expert")]
    HushHushExpert,
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
            "Skill & Dedication" => MedalGroup::SkillDedication,
            "Hush-Hush" => MedalGroup::HushHush,
            "Hush-Hush (Expert)" => MedalGroup::HushHushExpert,
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
            Self::SkillDedication => "Skill & Dedication",
            Self::HushHush => "Hush-Hush",
            Self::HushHushExpert => "Hush-Hush (Expert)",
            Self::BeatmapPacks => "Beatmap Packs",
            Self::BeatmapChallengePacks => "Beatmap Challenge Packs",
            Self::SeasonalSpotlights => "Seasonal Spotlights",
            Self::BeatmapSpotlights => "Beatmap Spotlights",
            Self::ModIntroduction => "Mod Introduction",
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

impl ArchivedOsekaiMedal {
    /// Returns a properly encoded medal url to osekai.
    pub fn url(&self) -> Result<String> {
        OsekaiMedal::name_to_url(self.name.as_ref())
    }

    /// Returns a backup url in case [`ArchivedOsekaiMedal::url`] fails.
    pub fn backup_url(&self) -> String {
        OsekaiMedal::backup_name_to_url(self.name.as_ref())
    }

    /// Returns the solution of the medal, if available.
    ///
    /// All content inbetween brackets (`<>`) is removed.
    pub fn solution(&self) -> Option<Cow<'_, str>> {
        // The Internment medal solution contains CSS and is too long
        // so instead of solving it programmatically, it'll just be hardcoded.
        if self.medal_id == INTERNMENT_ID {
            return Some(Cow::Borrowed(INTERNMENT_SOLUTION));
        }

        let solution = self.solution.as_deref()?;

        let mut res = Cow::<'_, str>::default();
        let mut stack = Vec::new();
        let mut brackets = Vec::new();

        struct Bracket {
            open: usize,
            close: usize,
        }

        for (i, c) in solution.char_indices() {
            match c {
                '<' => stack.push(i),
                '>' => {
                    if let Some(open) = stack.pop() {
                        brackets.push(Bracket { open, close: i });
                    }
                }
                _ => {}
            }
        }

        let mut last_close = 0;

        if !brackets.is_empty() {
            brackets.sort_unstable_by_key(|bracket| bracket.open);

            for Bracket { open, close } in brackets {
                if close < last_close {
                    continue;
                }

                // SAFETY: Indices are guaranteed to be within bounds
                res += unsafe { solution.get_unchecked(last_close..open) };
                last_close = close + 1;
            }
        }

        // SAFETY: Index is guaranteed to be within bounds
        res += unsafe { solution.get_unchecked(last_close..) };

        Some(res)
    }

    pub fn icon_url(&self) -> OsekaiMedalIconUrl<'_> {
        OsekaiMedalIconUrl {
            filename: self.icon_url_suffix.as_ref(),
        }
    }
}

impl OsekaiMedal {
    const BASE_URL: &'static str = "https://osekai.net/medals?";

    /// Returns a properly encoded medal url to osekai.
    pub fn url(&self) -> Result<String> {
        Self::name_to_url(self.name.as_ref())
    }

    /// Returns a backup url in case [`OsekaiMedal::url`] fails.
    pub fn backup_url(&self) -> String {
        Self::backup_name_to_url(self.name.as_ref())
    }

    pub fn backup_name_to_url(name: &str) -> String {
        format!("{}medal={name}", Self::BASE_URL)
    }

    pub fn name_to_url(name: &str) -> Result<String> {
        let mut url = String::with_capacity(Self::BASE_URL.len() + "medal".len() + 1 + name.len());
        url.push_str(Self::BASE_URL);

        #[derive(serde::Serialize)]
        struct MedalUrlQuery<'a> {
            medal: &'a str,
        }

        let query = MedalUrlQuery { medal: name };
        let mut form_serializer = FormSerializer::for_suffix(&mut url, Self::BASE_URL.len());
        let url_serializer = UrlSerializer::new(&mut form_serializer);

        query
            .serialize(url_serializer)
            .wrap_err("Failed to encode medal url")?;

        Ok(url)
    }

    fn grouping_order(&self) -> u32 {
        self.grouping.order()
    }
}

pub struct OsekaiMedalIconUrl<'a> {
    filename: &'a str,
}

impl Display for OsekaiMedalIconUrl<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "https://inex.osekai.net/assets/osu/web/{}",
            self.filename
        )
    }
}

const INTERNMENT_ID: u32 = 323;

const INTERNMENT_SOLUTION: &str = "On any 'Insane' difficulty (4.0\\* - 5.29\\*) of *`Frums - theyaremanycolors`*, \
set three plays in a row that have combos equal to the R, G, and B values \
of the difficulty indicator (on the osu! website) for the map you are playing.\n\
The possible combinations are as follows:\n\
```\n\
Play | Combo (vanchanical) | Combo (celi)\n\
-----+---------------------+-------------\n\
1st  | 255x                | 243x\n\
2nd  | 104x                | 76x\n\
3rd  | 108x                | 133x\n\
```\n\
You can find an explanation to the solution in the pinned comments.\n\
NOTE: You **must** get the scores back-to-back, and the third one must be a pass; \
if you overcombo at any point on the second or third play, fail the third play, \
or fail to reach the max combo requirement for the second or third score, \
you must restart the medal from the beginning.\n\
NOTE: Difficulty reduction mods **are** allowed for the first two plays, \
despite the category the medal is in.";

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

fn medal_mods<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Box<str>>, D::Error> {
    <Option<Box<str>> as Deserialize>::deserialize(d).map(|opt| opt.filter(|mods| !mods.is_empty()))
}

fn osekai_mode<'de, D: Deserializer<'de>>(d: D) -> Result<GameMode, D::Error> {
    maybe_osekai_mode(d)?.ok_or_else(|| Error::custom("missing mode"))
}

fn maybe_osekai_mode<'de, D: Deserializer<'de>>(d: D) -> Result<Option<GameMode>, D::Error> {
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
                    ));
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

#[derive(Archive, Debug, RkyvDeserialize, Serialize)]
#[rkyv(as = ArchivedOsekaiRankingEntry<T>)]
pub struct OsekaiRankingEntry<T: Archive> {
    #[rkyv(with = DerefAsString)]
    pub country: Box<str>,
    #[rkyv(with = DerefAsString)]
    pub country_code: CountryCode,
    pub rank: u32,
    pub user_id: u32,
    #[rkyv(with = DerefAsString)]
    pub username: Username,
    value: ValueWrapper<T>,
}

#[derive(Portable, CheckBytes)]
#[bytecheck(crate = rkyv::bytecheck)]
#[repr(C)]
pub struct ArchivedOsekaiRankingEntry<T: Archive> {
    pub country: ArchivedString,
    pub country_code: ArchivedString,
    pub rank: Archived<u32>,
    pub user_id: Archived<u32>,
    pub username: ArchivedString,
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

#[derive(Archive, Copy, Clone, RkyvDeserialize, Serialize, Portable, CheckBytes)]
#[rkyv(as = ValueWrapper<T::Archived>)]
#[bytecheck(crate = rkyv::bytecheck)]
#[repr(C)]
pub struct ValueWrapper<T>(T);

impl<T: Debug> Debug for ValueWrapper<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <T as Debug>::fmt(&self.0, f)
    }
}

impl<T: Display> Display for ValueWrapper<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <T as Display>::fmt(&self.0, f)
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
    #[rkyv(with = DerefAsString)]
    pub country_code: CountryCode,
    #[rkyv(with = DerefAsString)]
    pub country: Box<str>,
    #[rkyv(with = DerefAsString)]
    pub username: Username,
    #[serde(rename = "medalCount", with = "deser::u32_string")]
    pub medal_count: u32,
    #[serde(rename = "rarestmedal")]
    #[rkyv(with = DerefAsString)]
    pub rarest_medal: Box<str>,
    #[serde(rename = "link")]
    #[rkyv(with = DerefAsString)]
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
    #[rkyv(with = DerefAsString)]
    pub icon_url: Box<str>,
    #[serde(rename = "medalname")]
    #[rkyv(with = DerefAsString)]
    pub medal_name: Box<str>,
    #[serde(rename = "medalid", with = "deser::u32_string")]
    pub medal_id: u32,
    #[rkyv(with = DerefAsString)]
    pub description: Box<str>,
    #[serde(rename = "possessionRate", with = "deser::f32_string")]
    pub possession_percent: f32,
    #[serde(rename = "gameMode", deserialize_with = "maybe_osekai_mode")]
    #[rkyv(with = NicheInto<GameModeNiche>)]
    pub mode: Option<GameMode>,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, Serialize)]
pub struct OsekaiBadge {
    #[serde(with = "deser::date")]
    #[rkyv(with = DateRkyv)]
    pub awarded_at: Date,
    #[rkyv(with = DerefAsString)]
    pub description: Box<str>,
    #[serde(rename = "id", with = "deser::u32_string")]
    pub badge_id: u32,
    #[rkyv(with = DerefAsString)]
    pub image_url: Box<str>,
    #[rkyv(with = DerefAsString)]
    pub name: Box<str>,
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
    pub country_code: CountryCode,
    #[serde(rename = "id")]
    pub user_id: u32,
    #[serde(rename = "name")]
    pub username: Username,
}

#[derive(Deserialize)]
pub struct OsekaiInex<T> {
    // pub success: bool,
    // pub message: String,
    pub content: T,
}
