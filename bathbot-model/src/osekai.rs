use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    str::FromStr,
};

use bathbot_util::CowUtils;
use eyre::Result;
use rkyv::{
    Archive, Deserialize as RkyvDeserialize, Portable, Serialize as RkyvSerialize,
    bytecheck::CheckBytes,
    niche::niching::{NaN, Null},
    with::{Map, NicheInto},
};
use rosu_v2::{
    model::GameMode,
    prelude::{CountryCode, Username},
};
use serde::{
    Deserialize, Deserializer,
    de::{Error, Unexpected, Visitor},
};
use time::OffsetDateTime;
use twilight_interactions::command::{CommandOption, CreateOption};

use super::deser;
use crate::{
    RankingKind,
    rkyv_util::{DerefAsString, time::DateTimeRkyv},
    rosu_v2::mode::GameModeNiche,
};

pub trait OsekaiRanking {
    const RANKING: RankingKind;
    const KIND: &'static str;
    const OPTIONS_KIND: Option<&'static str>;
}

macro_rules! define_ranking {
    (
        $struct:ident {
            $ranking:ident,
            $kind:literal
            $(, $options_kind:literal )?
        } $( -> $entry:ident )?
    ) => {
        pub struct $struct;

        impl OsekaiRanking for $struct {
            const RANKING: RankingKind = RankingKind::$ranking;
            const KIND: &'static str = $kind;
            const OPTIONS_KIND: Option<&'static str> = define_ranking!(@options $($options_kind)?);
        }
    };
    ( @options $options:literal ) => {
        Some($options)
    };
    ( @options ) => {
        None
    };
}

define_ranking! {
    Rarity { OsekaiRarity, "medals_rarity" } -> OsekaiRarityEntry
}

define_ranking! {
    MedalCount { OsekaiMedalCount, "medals_users" }
}

define_ranking! {
    Replays { OsekaiReplays, "replays" }
}

define_ranking! {
    TotalPp { OsekaiTotalPp, "pp", "total" }
}

define_ranking! {
    StandardDeviation { OsekaiStandardDeviation, "pp", "stdev" }
}

define_ranking! {
    Badges { OsekaiBadges, "badges" }
}

define_ranking! {
    RankedMapsets { OsekaiRankedMapsets, "mapsets", "ranked" }
}

define_ranking! {
    LovedMapsets { OsekaiLovedMapsets, "mapsets", "loved" }
}

define_ranking! {
    Subscribers { OsekaiSubscribers, "subscribers" }
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

#[derive(Archive, Clone, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
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
    RkyvSerialize,
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
    pub fn url(&self) -> String {
        OsekaiMedal::name_to_url(self.name.as_ref())
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
    const BASE_URL: &'static str = "https://inex.osekai.net/medals/";

    /// Returns a properly encoded medal url to osekai.
    pub fn url(&self) -> String {
        Self::name_to_url(self.name.as_ref())
    }

    pub fn name_to_url(name: &str) -> String {
        format!("{}{}", Self::BASE_URL, name.cow_replace(' ', "%20"))
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

#[derive(Deserialize)]
pub struct OsekaiRankingEntries<T> {
    pub data: CompactWrap<T>,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsekaiUserEntry {
    #[serde(rename = "Rank")]
    rank: u32,
    #[serde(rename = "ID")]
    pub user_id: u32,
    #[serde(rename = "Accuracy_Catch", with = "deser::f32_string")]
    acc_catch: f32,
    #[serde(rename = "Accuracy_Mania", with = "deser::f32_string")]
    acc_mania: f32,
    #[serde(rename = "Accuracy_Standard", with = "deser::f32_string")]
    acc_standard: f32,
    #[serde(rename = "Accuracy_Taiko", with = "deser::f32_string")]
    acc_taiko: f32,
    #[serde(rename = "Accuracy_Stdev", with = "deser::f32_string")]
    acc_stdev: f32,
    #[serde(rename = "Count_Badges")]
    count_badges: u32,
    #[serde(rename = "Count_Maps_Loved")]
    count_maps_loved: u32,
    #[serde(rename = "Count_Maps_Ranked")]
    count_maps_ranked: u32,
    #[serde(rename = "Count_Medals")]
    pub count_medals: u32,
    #[serde(rename = "Count_Replays_Watched")]
    count_replays_watched: u64,
    #[serde(rename = "Count_Subscribers")]
    count_subscribers: u32,
    #[serde(rename = "Country_Code")]
    #[rkyv(with = DerefAsString)]
    pub country_code: CountryCode,
    #[serde(rename = "Is_Restricted", with = "deser::bool_as_u8")]
    is_restricted: bool,
    #[serde(rename = "Level_Catch")]
    level_catch: u16,
    #[serde(rename = "Level_Mania")]
    level_mania: u16,
    #[serde(rename = "Level_Standard")]
    level_standard: u16,
    #[serde(rename = "Level_Taiko")]
    level_taiko: u16,
    #[serde(rename = "Level_Stdev")]
    level_stdev: u16,
    #[serde(rename = "Name")]
    #[rkyv(with = DerefAsString)]
    pub username: Username,
    #[serde(rename = "PP_Catch", with = "deser::f32_string")]
    pp_catch: f32,
    #[serde(rename = "PP_Mania", with = "deser::f32_string")]
    pp_mania: f32,
    #[serde(rename = "PP_Standard", with = "deser::f32_string")]
    pp_standard: f32,
    #[serde(rename = "PP_Taiko", with = "deser::f32_string")]
    pp_taiko: f32,
    #[serde(rename = "PP_Stdev", with = "deser::f32_string")]
    pp_stdev: f32,
    #[serde(rename = "PP_Total", with = "deser::f32_string")]
    pp_total: f32,
    #[serde(rename = "Rank_Global_Catch")]
    rank_global_catch: Option<u32>,
    #[serde(rename = "Rank_Global_Mania")]
    rank_global_mania: Option<u32>,
    #[serde(rename = "Rank_Global_Standard")]
    rank_global_standard: Option<u32>,
    #[serde(rename = "Rank_Global_Taiko")]
    rank_global_taiko: Option<u32>,
    #[serde(rename = "Rarest_Medal_Achieved", with = "deser::naive_datetime")]
    #[rkyv(with = DateTimeRkyv)]
    rarest_medal_achieved: OffsetDateTime,
    #[serde(rename = "Rarest_Medal_ID")]
    rarest_medal_id: u16,
    /// How many people achieved the rarest medal
    #[serde(rename = "Rarest_Medal_Frequency")]
    rarest_medal_frequency: u32,
    #[serde(rename = "Medal_Data")]
    pub rarest_medal: OsekaiRankingEntryMedal,
    #[serde(rename = "Medal_Percentage", with = "deser::f32_string")]
    pub medal_percentage: f32,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsekaiRankingEntry {
    #[serde(rename = "Rank")]
    rank: u32,
    #[serde(rename = "ID")]
    user_id: u32,
    #[serde(rename = "Accuracy_Catch", with = "deser::f32_string")]
    acc_catch: f32,
    #[serde(rename = "Accuracy_Mania", with = "deser::f32_string")]
    acc_mania: f32,
    #[serde(rename = "Accuracy_Standard", with = "deser::f32_string")]
    acc_standard: f32,
    #[serde(rename = "Accuracy_Taiko", with = "deser::f32_string")]
    acc_taiko: f32,
    #[serde(rename = "Accuracy_Stdev", with = "deser::f32_string")]
    acc_stdev: f32,
    #[serde(rename = "Count_Badges")]
    pub count_badges: u32,
    #[serde(rename = "Count_Maps_Loved")]
    pub count_maps_loved: u32,
    #[serde(rename = "Count_Maps_Ranked")]
    pub count_maps_ranked: u32,
    #[serde(rename = "Count_Medals")]
    count_medals: u32,
    #[serde(rename = "Count_Replays_Watched")]
    pub count_replays_watched: u64,
    #[serde(rename = "Count_Subscribers")]
    pub count_subscribers: u32,
    #[serde(rename = "Country_Code")]
    #[rkyv(with = DerefAsString)]
    pub country_code: CountryCode,
    #[serde(rename = "Is_Restricted", with = "deser::bool_as_u8")]
    is_restricted: bool,
    #[serde(rename = "Level_Catch")]
    level_catch: u16,
    #[serde(rename = "Level_Mania")]
    level_mania: u16,
    #[serde(rename = "Level_Standard")]
    level_standard: u16,
    #[serde(rename = "Level_Taiko")]
    level_taiko: u16,
    #[serde(rename = "Level_Stdev")]
    level_stdev: u16,
    #[serde(rename = "Name")]
    #[rkyv(with = DerefAsString)]
    pub username: Username,
    #[serde(rename = "PP_Catch", with = "deser::f32_string")]
    pp_catch: f32,
    #[serde(rename = "PP_Mania", with = "deser::f32_string")]
    pp_mania: f32,
    #[serde(rename = "PP_Standard", with = "deser::f32_string")]
    pp_standard: f32,
    #[serde(rename = "PP_Taiko", with = "deser::f32_string")]
    pp_taiko: f32,
    #[serde(rename = "PP_Stdev", with = "deser::f32_string")]
    pub pp_stdev: f32,
    #[serde(rename = "PP_Total", with = "deser::f32_string")]
    pub pp_total: f32,
    #[serde(rename = "Rank_Global_Catch")]
    rank_global_catch: Option<u32>,
    #[serde(rename = "Rank_Global_Mania")]
    rank_global_mania: Option<u32>,
    #[serde(rename = "Rank_Global_Standard")]
    rank_global_standard: Option<u32>,
    #[serde(rename = "Rank_Global_Taiko")]
    rank_global_taiko: Option<u32>,
    #[serde(rename = "Rarest_Medal_Achieved", with = "deser::naive_datetime")]
    #[rkyv(with = DateTimeRkyv)]
    rarest_medal_achieved: OffsetDateTime,
    #[serde(rename = "Rarest_Medal_ID")]
    rarest_medal_id: u16,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsekaiRankingEntryMedal {
    #[serde(rename = "Medal_ID")]
    medal_id: u16,
    #[serde(rename = "Name")]
    #[rkyv(with = DerefAsString)]
    pub name: Box<str>,
    #[serde(rename = "Frequency", deserialize_with = "adjust_frequency")]
    frequency: f64,
    #[serde(rename = "Count_Achieved_By")]
    count_achieved_by: u32,
}

fn adjust_frequency<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(<f64 as Deserialize>::deserialize(d)? * 100.0)
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsekaiRarityEntry {
    // #[serde(rename = "Rank")]
    // rank: u32,
    #[serde(rename = "Medal_ID")]
    pub medal_id: u16,
    #[serde(rename = "Name")]
    #[rkyv(with = DerefAsString)]
    pub medal_name: Box<str>,
    // #[serde(rename = "Link")]
    // #[rkyv(with = DerefAsString)]
    // link: Box<str>,
    #[serde(rename = "Description")]
    #[rkyv(with = DerefAsString)]
    pub description: Box<str>,
    // #[serde(rename = "Gamemode")]
    // #[rkyv(with = NicheInto<GameModeNiche>)]
    // mode: Option<GameMode>,
    // #[serde(rename = "Grouping")]
    // #[rkyv(with = DerefAsString)]
    // grouping: Box<str>,
    // #[serde(rename = "Instructions")]
    // #[rkyv(with = Map<DerefAsString>)]
    // instructions: Option<Box<str>>,
    // #[serde(rename = "Ordering")]
    // ordering: u8,
    #[serde(rename = "Frequency", deserialize_with = "adjust_frequency")]
    pub frequency: f64,
    #[serde(rename = "Count_Achieved_By")]
    pub count_achieved_by: u32,
    // #[serde(rename = "Achieved")]
    // achieved: bool,
}

#[derive(Deserialize)]
pub struct OsekaiBadges(#[serde(with = "compact")] pub Vec<OsekaiBadge>);

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsekaiBadge {
    #[serde(rename = "First_Date_Awarded", with = "deser::naive_datetime")]
    #[rkyv(with = DateTimeRkyv)]
    pub first_date_awarded: OffsetDateTime,
    #[serde(rename = "Description")]
    #[rkyv(with = DerefAsString)]
    pub description: Box<str>,
    #[serde(rename = "ID", with = "deser::u32_string")]
    pub badge_id: u32,
    #[serde(rename = "Image_URL")]
    #[rkyv(with = DerefAsString)]
    pub image_url: Box<str>,
    #[serde(rename = "Name")]
    #[rkyv(with = DerefAsString)]
    pub name: Box<str>,
    #[serde(rename = "Users", with = "compact")]
    pub users: Vec<OsekaiBadgeUser>,
}

#[derive(Archive, Debug, Deserialize, RkyvDeserialize, RkyvSerialize)]
pub struct OsekaiBadgeUser {
    #[serde(rename = "User_ID")]
    pub user_id: u32,
    #[serde(rename = "Username")]
    #[rkyv(with = DerefAsString)]
    pub username: Box<str>,
    // TODO: remove `Option` once API includes country_code
    #[serde(rename = "Country_Code")]
    #[rkyv(with = Map<DerefAsString>)]
    pub country_code: Option<CountryCode>,
}

#[derive(Deserialize)]
pub struct OsekaiInex<T> {
    pub content: T,
}

#[derive(Deserialize)]
#[serde(bound = "T: Deserialize<'de>")]
pub struct CompactWrap<T>(#[serde(with = "compact")] pub Vec<T>);

mod compact {
    //! Module to deserialize a compact array-based JSON format.
    //!
    //! This module provides a [`deserialize`](`crate::compact::deserialize`)
    //! function that can be used with `#[serde(with = "compact")]` to
    //! deserialize fields from a compact format into standard Rust types.
    //!
    //! # Compact Format
    //!
    //! The compact format encodes structured data as nested arrays instead of
    //! JSON objects. Each compact object has two special fields:
    //!
    //! - `"k"` — a JSON array of field name strings (keys)
    //! - `"d"` — a JSON array of corresponding values (data)
    //!
    //! Values under `"d"` can themselves be compact objects, enabling arbitrary
    //! nesting. Extra fields (anything other than `"k"` or `"d"`) are silently
    //! ignored.
    //!
    //! # Example
    //!
    //! ```text
    //! // Compact format:
    //! // { "k": ["id", "name"], "d": [["42", "peppy"]] }
    //!
    //! // Deserializes to:
    //! // vec![MyStruct { id: 42, name: "peppy".into() }]
    //! ```
    //!
    //! # Implementation Details
    //!
    //! The module implements the serde deserializer traits (`DeserializeSeed`,
    //! `Visitor`, `MapAccess`) to interpret the compact arrays as key-value
    //! pairs. The `'k'` field must appear before `'d'` in the JSON object.

    use std::{
        error::Error,
        fmt::{Debug, Display, Formatter, Result as FmtResult},
        marker::PhantomData,
        slice,
    };

    use serde::{de, forward_to_deserialize_any};

    /// Deserializes a compact-format JSON value into a `Vec<T>`.
    ///
    /// This function is intended to be used with serde's `#[serde(with = "compact")]`
    /// attribute. It expects a JSON object containing `"k"` (keys) and `"d"` (data)
    /// fields and produces a vector of deserialized `T` values.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `"d"` field is missing
    /// - The `"k"` field does not appear before `"d"`
    /// - Any element in the data array fails to deserialize as `T`
    pub fn deserialize<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
    where
        D: de::Deserializer<'de>,
        T: de::Deserialize<'de>,
    {
        /// Deserializes a sequence of compact objects into a `Vec<T>`.
        ///
        /// Each element in the sequence is processed by a [`Compact`] seed
        /// that maps array positions to field names from `keys`.
        struct CompactList<'a, 'de, T> {
            /// Field names that map to positions in each data array.
            keys: &'a [&'de str],
            _phantom: PhantomData<T>,
        }

        impl<'de, T: de::Deserialize<'de>> de::DeserializeSeed<'de> for CompactList<'_, 'de, T> {
            type Value = <Self as de::Visitor<'de>>::Value;

            fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
                d.deserialize_seq(self)
            }
        }

        impl<'de, T: de::Deserialize<'de>> de::Visitor<'de> for CompactList<'_, 'de, T> {
            type Value = Vec<T>;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("a list of compact data")
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut data = Vec::with_capacity(seq.size_hint().unwrap_or(0));

                let seed = Compact {
                    keys: self.keys,
                    _phantom: PhantomData,
                };

                while let Some(item) = seq.next_element_seed(seed)? {
                    data.push(item);
                }

                Ok(data)
            }
        }

        /// A seed that deserializes a single compact object (array of values)
        /// into `T`.
        ///
        /// Uses the `keys` slice to map each array position to a field name,
        /// which is then passed to `T::deserialize` via a [`CompactDeserializer`]
        /// that presents the array as a map-like interface.
        struct Compact<'a, 'de, T> {
            /// Field names that map to positions in the data array.
            keys: &'a [&'de str],
            _phantom: PhantomData<T>,
        }

        impl<T> Clone for Compact<'_, '_, T> {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl<T> Copy for Compact<'_, '_, T> {}

        impl<'de, T: de::Deserialize<'de>> de::DeserializeSeed<'de> for Compact<'_, 'de, T> {
            type Value = <Self as de::Visitor<'de>>::Value;

            fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
                d.deserialize_seq(self)
            }
        }

        impl<'de, T: de::Deserialize<'de>> de::Visitor<'de> for Compact<'_, 'de, T> {
            type Value = T;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("a compact object")
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
                let d = CompactDeserializer {
                    keys: self.keys.iter(),
                    seq,
                };

                T::deserialize(d).map_err(de::Error::custom)
            }
        }

        /// Implements [`MapAccess`] to present a compact data array as a map.
        ///
        /// Iterates over `keys` to produce field names, pulling corresponding
        /// values from the underlying sequence access.
        struct CompactDeserializer<'a, 'de, A> {
            /// Iterator over field names.
            keys: slice::Iter<'a, &'de str>,
            /// The underlying sequence providing values.
            seq: A,
        }

        /// Error type for [`CompactDeserializer`] operations.
        #[derive(Debug)]
        enum CompactDeserializerError<E> {
            /// An error originating from the underlying sequence access.
            Seq(E),
            /// A value was expected but the sequence was exhausted.
            MissingValue,
            /// A custom error with a message.
            Custom(String),
        }

        impl<E> From<E> for CompactDeserializerError<E> {
            fn from(err: E) -> Self {
                Self::Seq(err)
            }
        }

        impl<E: de::Error> de::Error for CompactDeserializerError<E> {
            fn custom<T: Display>(msg: T) -> Self {
                Self::Custom(msg.to_string())
            }
        }

        impl<E: Error> Error for CompactDeserializerError<E> {}

        impl<E: Display> Display for CompactDeserializerError<E> {
            fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
                match self {
                    CompactDeserializerError::Seq(err) => Display::fmt(err, f),
                    CompactDeserializerError::MissingValue => f.write_str("missing value"),
                    CompactDeserializerError::Custom(msg) => f.write_str(msg),
                }
            }
        }

        impl<'de, A: de::SeqAccess<'de>> de::Deserializer<'de> for CompactDeserializer<'_, 'de, A> {
            type Error = CompactDeserializerError<A::Error>;

            fn deserialize_map<V: de::Visitor<'de>>(self, v: V) -> Result<V::Value, Self::Error> {
                v.visit_map(self)
            }

            fn deserialize_any<V: de::Visitor<'de>>(self, v: V) -> Result<V::Value, Self::Error> {
                self.deserialize_map(v)
            }

            forward_to_deserialize_any! {
                bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str
                string bytes byte_buf option unit unit_struct newtype_struct seq
                tuple tuple_struct struct enum identifier ignored_any
            }
        }

        impl<'de, A: de::SeqAccess<'de>> de::MapAccess<'de> for CompactDeserializer<'_, 'de, A> {
            type Error = CompactDeserializerError<A::Error>;

            fn next_key_seed<K: de::DeserializeSeed<'de>>(
                &mut self,
                seed: K,
            ) -> Result<Option<K::Value>, Self::Error> {
                let Some(key) = self.keys.next() else {
                    return Ok(None);
                };

                seed.deserialize(de::value::StrDeserializer::new(key))
                    .map(Some)
            }

            fn next_value_seed<V: de::DeserializeSeed<'de>>(
                &mut self,
                seed: V,
            ) -> Result<V::Value, Self::Error> {
                self.seq
                    .next_element_seed(seed)?
                    .ok_or(CompactDeserializerError::MissingValue)
            }
        }

        /// Visitor for the top-level compact object.
        ///
        /// Extracts the `"k"` (keys) and `"d"` (data) fields from a JSON map,
        /// then delegates data deserialization to a [`CompactList`].
        struct CompactVisitor<T>(PhantomData<T>);

        impl<'de, T: de::Deserialize<'de>> de::Visitor<'de> for CompactVisitor<T> {
            type Value = Vec<T>;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("an object of compact data")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut keys: Option<Vec<&'de str>> = None;
                let mut data: Option<Vec<T>> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "k" => {
                            if keys.is_some() || data.is_some() {
                                return Err(de::Error::duplicate_field("k"));
                            }

                            keys = Some(map.next_value()?)
                        }
                        "d" => {
                            if data.is_some() {
                                return Err(de::Error::duplicate_field("d"));
                            }

                            let keys = keys
                                .take()
                                .ok_or_else(|| de::Error::custom("'k' must come before 'd'"))?;

                            let seed = CompactList {
                                keys: &keys,
                                _phantom: PhantomData,
                            };

                            data = Some(map.next_value_seed(seed)?);
                        }
                        _ => _ = map.next_value::<de::IgnoredAny>()?,
                    }
                }

                data.ok_or_else(|| de::Error::missing_field("d"))
            }
        }

        d.deserialize_map(CompactVisitor(PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_medal_count_ranking() {
        const JSON: &'static str = r#"{"_dev":"Compressed response. Remove 'compress' parameter for human-readable form.","success":true,"message":"ok","content":{"data":{"_t":true,"k":["Rank","ID","Accuracy_Catch","Accuracy_Mania","Accuracy_Standard","Accuracy_Stdev","Accuracy_Taiko","Count_Badges","Count_Maps_Loved","Count_Maps_Ranked","Count_Medals","Count_Replays_Watched","Count_Subscribers","Country_Code","Is_Restricted","Level_Catch","Level_Mania","Level_Standard","Level_Stdev","Level_Taiko","Name","PP_Catch","PP_Mania","PP_Standard","PP_Stdev","PP_Taiko","PP_Total","Rank_Global_Catch","Rank_Global_Mania","Rank_Global_Standard","Rank_Global_Taiko","Rarest_Medal_Achieved","Rarest_Medal_ID","Rarest_Medal_Frequency","Medal_Data","Medal_Percentage"],"d":[[1,4687701,"99.50","97.13","98.59","390.72","97.61",1,1,3,347,72392,150,"BR",0,100,86,104,373,97,"Dropinx","10796.70","7338.85","15386.90","38575.70","11664.30","45186.75",685,14926,541,964,"2020-04-25 00:26:56",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"100.00"],[2,9767342,"99.85","96.82","98.12","389.08","97.06",1,0,1,345,4638,15,"TW",0,101,80,102,356,94,"Flyer","16568.60","5897.14","13406.60","31905.65","6511.06","42383.40",182,25222,1406,4679,"2023-12-03 18:27:53",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"99.42"],[3,17274052,"99.09","95.87","95.98","381.67","94.58",0,0,0,342,26130,43,"US",0,127,76,101,292,53,"TheMagicAnimals","4348.25","2535.65","13135.10","14997.75","4490.40","24509.40",3250,108133,1627,8607,"2022-10-17 19:20:03",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"98.56"],[4,11883100,"99.44","97.10","98.91","390.22","97.16",0,0,0,341,50,0,"AU",0,100,85,101,358,89,"XimperiaL","9239.01","5123.36","10547.10","29094.74","8838.14","33747.61",921,34488,5765,2349,"2026-05-02 01:01:25",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"98.27"],[5,11005307,"99.44","95.61","98.83","387.98","97.48",0,0,0,340,923,1,"US",0,98,93,102,371,89,"sangoose","8602.43","5049.50","12001.90","29139.93","9200.13","34853.96",1066,35566,2836,2112,"2023-02-08 01:23:56",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"97.98"],[6,14269506,"98.86","96.00","97.81","386.19","96.23",1,0,26,340,802,79,"SK",0,94,76,101,346,100,"nevqr","4234.23","2834.38","14735.00","25022.15","19174.20","40977.81",3387,94342,729,63,"2022-11-05 21:10:20",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"97.98"],[7,13264148,"99.66","96.82","99.34","391.54","98.28",0,0,0,339,1352,4,"DE",0,99,79,103,327,75,"Magma","8350.22","2364.95","9826.84","21194.97","7111.04","27653.05",1141,117090,8494,3916,"2022-02-16 16:17:23",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"97.69"],[8,6303313,"99.53","97.52","98.93","392.79","98.49",12,0,0,338,301177,41,"US",0,95,95,104,382,97,"Mathyu","4217.08","13454.80","14217.90","35297.20","12739.50","44629.28",3403,1443,936,670,"2023-06-01 07:49:03",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"97.41"],[9,10218998,"99.94","96.42","99.35","389.87","97.43",0,0,0,338,4783,10,"CL",0,103,96,102,360,80,"Bomb","13890.60","5891.10","7167.02","24077.04","5133.13","32081.85",348,25274,32937,7056,"2022-08-12 05:03:37",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"97.41"],[10,20512411,"99.67","96.43","97.16","386.98","96.69",0,0,1,338,50,3,"SK",0,99,72,101,324,80,"micqaal","10040.90","7975.71","10994.10","36655.49","10224.70","39235.41",793,11969,4591,1543,"2025-06-26 16:51:15",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"97.41"],[11,7183087,"99.53","95.44","97.48","386.29","97.19",0,0,4,337,979,12,"GB",0,96,80,101,357,100,"mareep","4650.47","3083.31","7473.22","19534.21","13594.10","28801.10",2921,84347,28003,513,"2023-08-27 00:12:03",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"97.12"],[12,9507660,"99.40","96.24","99.12","390.38","98.48",0,0,0,337,36589,8,"BE",0,100,84,103,339,77,"Dabo","6491.37","1884.42","10708.50","14319.50","3132.60","22216.89",1801,149170,5284,13086,"2020-04-28 13:30:33",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"97.12"],[13,10238680,"99.41","96.25","98.80","390.83","99.36",1,0,1,336,323,31,"GB",0,97,84,101,363,97,"chromb","4425.50","4580.07","7679.14","21313.21","9909.32","26594.03",3150,43395,25064,1707,"2021-11-20 22:34:49",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"96.83"],[14,11354436,"99.69","96.94","98.99","391.42","98.16",3,0,9,336,1132,54,"US",0,100,90,101,288,48,"Utiba","9275.65","8722.76","7143.20","26265.57","4977.27","30118.88",911,9040,33387,7405,"2024-11-27 20:20:52",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"96.83"],[15,10615367,"99.84","96.91","99.03","391.72","98.42",0,0,0,336,582,7,"DE",0,101,99,100,388,94,"ERA Pheon","16341.90","10057.50","7252.83","36547.07","10526.50","44178.73",194,5531,31529,1432,"2025-04-21 12:56:40",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"96.83"],[16,9079969,"99.60","96.21","98.09","389.07","97.94",0,0,3,335,313,11,"PL",0,100,84,101,346,82,"Koxiuuu","7379.74","2130.81","9902.95","21942.36","9816.92","29230.42",1451,131274,8182,1759,"2023-10-17 16:57:49",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"96.54"],[17,13588932,"99.44","96.01","99.38","388.63","97.18",0,0,0,335,1172,12,"NZ",0,84,93,102,271,44,"pii","3209.98","2797.62","14655.80","12390.49","3291.33","23954.73",5035,95964,750,12379,"2026-03-21 03:21:26",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"96.54"],[18,15769399,"99.38","96.86","99.15","391.02","97.96",0,0,0,335,26,2,"US",0,99,86,101,346,80,"testie","6603.88","7002.78","9314.57","29373.81","9450.76","32371.99",1757,16812,11108,1949,"2025-01-27 05:50:14",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"96.54"],[19,17957276,"98.65","97.64","97.67","387.91","96.08",0,0,0,335,350,2,"NL",0,92,93,100,339,75,"- joshh","3584.33","8366.75","6788.78","22594.25","8360.30","27100.16",4277,10377,40269,2670,"2024-12-01 13:12:29",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"96.54"],[20,8509247,"99.58","96.10","98.86","390.28","98.81",0,0,0,334,638,7,"BE",0,99,82,102,364,99,"Aya Shameimaru","5834.93","2926.34","10538.90","18192.95","5272.31","24572.48",2117,90597,5796,6759,"2022-02-10 18:31:05",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"96.25"],[21,165027,"99.65","97.89","99.40","394.67","99.32",12,0,14,334,115816,81,"KR",0,100,92,100,384,100,"Peaceful","8640.12","10038.20","9022.71","36887.78","24042.20","51743.23",1059,5572,12800,6,"2025-04-13 13:31:26",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"96.25"],[22,10520757,"98.98","96.68","98.69","390.46","98.17",0,0,0,334,5884,21,"US",0,96,97,102,389,99,"WARABIMON","6668.15","10850.50","12432.40","37722.95","14225.00","44176.05",1718,3984,2279,414,"2025-04-13 20:41:59",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"96.25"],[23,13925852,"98.94","95.55","99.05","387.04","96.89",0,0,0,333,588,6,"BE",0,94,78,102,336,84,"brandwagen","4547.45","2698.71","9414.63","18700.80","8363.88","25024.67",3032,100348,10513,2667,"2023-02-24 12:31:19",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"95.97"],[24,14386290,"99.55","96.09","96.74","386.91","97.54",0,0,0,333,963,1,"FR",0,100,96,100,390,98,"Eloi2525","7288.74","7042.80","7029.27","28476.64","13529.50","34890.31",1488,16554,35488,521,"2024-01-05 20:27:37",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"95.97"],[25,12453848,"99.24","97.25","98.48","390.36","97.32",2,0,9,333,99,34,"FR",0,94,68,101,333,99,"Glassive","5372.69","3160.76","9404.29","22513.99","14801.50","32739.24",2380,81369,10584,346,"2025-04-13 12:46:20",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"95.97"],[26,23280447,"99.29","95.61","98.56","388.28","98.01",1,0,0,333,422,2,"JP",0,61,49,101,249,83,"hosesan1020","4457.43","4051.93","13855.10","24037.82","11658.90","34023.36",3122,54977,1121,967,"2025-05-18 06:38:15",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"95.97"],[27,6291386,"100.00","99.64","100.00","397.85","99.08",0,0,0,332,68771,19,"FI",0,88,76,102,267,47,"Tactic","1883.21","0.00","8046.62","6979.44","3952.46","13882.29",10504,null,20928,10136,"2021-11-26 16:07:10",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"95.68"],[28,12647024,"98.40","95.50","97.64","385.26","96.32",0,0,0,332,389,31,"RU",0,77,80,101,245,39,"anagor","3559.66","3001.28","11686.20","13115.29","3284.80","21531.94",4321,87682,3281,12410,"2025-11-26 23:29:58",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"95.68"],[29,1699875,"99.48","96.54","99.00","390.59","98.16",1,2,0,332,2177,13,"CA",0,100,94,101,377,91,"Remyria","4270.13","5600.95","9252.26","23090.56","8856.47","27979.81",3336,28387,11476,2335,"2025-04-13 12:49:25",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"95.68"],[30,4653583,"99.84","96.46","99.45","391.99","99.35",3,0,0,331,32290,17,"CL",0,98,75,106,350,98,"NO37","4415.56","3524.20","12031.00","21390.17","9602.95","29573.71",3160,69244,2793,1858,"2021-12-24 16:47:31",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"95.39"],[31,6735738,"99.22","96.01","98.72","389.07","97.93",2,0,0,331,2210,32,"ES",0,97,72,109,317,75,"A L E P H","4199.56","4634.27","16471.40","21115.99","7243.63","32548.86",3424,42416,329,3767,"2022-12-17 23:06:23",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"95.39"],[32,9169747,"99.89","98.25","98.07","390.14","96.61",2,0,1,331,44247,222,"GB",0,102,100,100,396,98,"Nathanial","20500.70","14039.90","9232.06","52469.76","18877.20","62649.86",43,1149,11602,76,"2025-05-04 18:25:14",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"95.39"],[33,6245906,"99.58","96.23","98.86","390.05","98.25",1,0,0,330,54766,30,"CA",0,99,88,109,335,71,"Furina-","7823.54","4150.21","10000.30","23995.65","6871.24","28845.29",1301,52601,7825,4190,"2020-06-15 20:41:56",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"95.10"],[34,15266747,"99.31","96.77","99.04","390.86","98.05",0,0,0,330,355,16,"GB",0,92,94,101,368,90,"--Xer0--","4748.80","9640.33","8070.04","27953.42","10664.90","33124.07",2826,6405,20706,1367,"2025-04-13 16:30:40",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"95.10"],[35,18867523,"96.93","98.54","95.90","383.68","95.21",2,0,0,330,1090,31,"FR",0,73,99,100,256,40,"flowerful","1558.12","17650.30","7519.65","15711.12","3375.06","30103.13",13055,220,27300,12035,"2025-05-13 22:31:03",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"95.10"],[36,8788058,"98.94","96.73","98.98","389.94","97.50",3,0,0,330,19900,12,"US",0,89,73,104,270,50,"Jakson","5238.23","6661.25","15731.50","23702.20","5928.72","33559.70",2465,18880,459,5542,"2024-08-22 13:47:58",192,180,{"Medal_ID":192,"Name":"Skylord","Link":"osu-secret-skylord.png","Frequency":6.40725e-6,"Count_Achieved_By":180},"95.10"],[37,4673649,"99.48","96.57","98.37","390.66","98.70",11,0,0,329,7464,11,"US",0,97,91,103,366,88,"LUNAISTABBY","5374.38","5365.83","10728.00","25535.06","9741.53","31209.74",2377,31240,5231,1793,"2022-06-22 00:02:39",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"94.81"],[38,10937890,"99.83","96.71","99.07","391.65","98.71",0,0,4,329,1264,17,"US",0,100,69,106,289,59,"yukinasimp","12669.00","4255.36","13547.80","25313.57","4790.56","35262.72",459,50215,1308,7854,"2024-08-04 03:48:26",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"94.81"],[39,18065598,"99.46","96.31","98.00","386.24","95.80",0,0,1,329,117,11,"CA",0,100,77,100,351,97,"azu420","8284.89","5393.26","6389.34","26138.91","11037.70","31105.19",1164,30894,49886,1227,"2022-07-25 00:39:50",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"94.81"],[40,10415491,"99.38","96.76","98.53","391.01","98.55",1,0,45,329,630,60,"VN",0,97,82,101,359,95,"Amasugi","5100.46","8095.11","9154.76","27672.15","9155.86","31506.19",2546,11458,11994,2139,"2025-04-14 03:25:35",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"94.81"],[41,18319269,"99.31","97.16","99.02","392.63","99.15",0,0,1,329,179,9,"US",0,97,78,102,339,84,"catgirl enjoyer","5086.22","3024.99","8666.17","17669.24","5553.34","22330.72",2560,86732,15316,6188,"2025-04-13 16:50:38",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"94.81"],[42,15787738,"99.70","97.55","97.91","391.44","98.18",0,0,0,328,281,10,"US",0,100,78,103,321,71,"Avias_","9725.43","5097.10","10305.90","24673.50","5190.86","30319.29",835,34869,6579,6918,"2025-01-23 06:31:42",344,257,{"Medal_ID":344,"Name":"Stargazer","Link":"taiko-secret-stargazer.png","Frequency":1.85148e-5,"Count_Achieved_By":257},"94.52"],[43,21138904,"99.80","96.35","87.07","369.21","97.05",1,0,1,328,779,11,"HK",0,205,84,102,361,86,"fua","14235.80","5331.99","9583.23","30857.37","9012.76","38163.78",322,31645,9606,2234,"2025-01-24 12:59:47",344,257,{"Medal_ID":344,"Name":"Stargazer","Link":"taiko-secret-stargazer.png","Frequency":1.85148e-5,"Count_Achieved_By":257},"94.52"],[44,10889664,"99.74","96.04","98.79","389.96","98.54",0,0,0,327,52,0,"GB",0,100,82,101,342,81,"Fretful4830","9299.17","3782.07","8506.47","24026.74","7311.48","28899.19",904,61821,16608,3681,"2025-08-12 11:22:26",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"94.24"],[45,14434221,"99.47","96.55","98.34","390.05","98.10",0,0,0,327,618,0,"US",0,85,66,102,271,57,"littleguy397658","4150.18","2838.63","10469.30","16645.84","5850.42","23308.53",3472,94178,6035,5672,"2025-04-13 15:04:49",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"94.24"],[46,15136054,"98.95","95.72","98.77","388.16","97.69",0,0,0,327,201,15,"RU",0,113,91,103,380,93,"Seijuro Akashi","6000.63","4816.85","8386.58","24020.60","8390.49","27594.55",2040,39243,17641,2654,"2025-04-15 04:34:28",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"94.24"],[47,15538779,"97.52","81.27","98.99","358.15","97.04",0,0,13,327,166,77,"US",0,50,79,101,235,54,"velamy","1198.54","410.38","7475.48","6366.85","3633.72","12718.12",17033,399945,27962,11148,"2025-04-14 04:31:58",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"94.24"],[48,9152166,"98.10","96.37","98.51","389.34","98.34",0,0,0,326,147,3,"US",0,81,88,101,343,91,"Mayano","3700.41","6138.50","6519.93","21090.63","9418.58","25777.42",4087,22967,46505,1986,"2025-04-13 10:50:29",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"93.95"],[49,12487726,"99.84","96.10","99.20","390.74","98.91",0,0,0,326,1569,3,"CA",0,100,93,103,334,69,"sumyeon","4784.61","6811.22","9412.23","21909.73","5128.20","26136.26",2797,17943,10530,7076,"2025-08-09 21:12:51",342,161,{"Medal_ID":342,"Name":"Pioneer","Link":"all-secret-pioneer.png","Frequency":5.73093e-6,"Count_Achieved_By":161},"93.95"],[50,9939642,"99.37","93.29","96.41","382.07","98.34",0,0,0,325,153,3,"CH",0,82,72,101,254,46,"Shina","3829.88","1284.87","7634.24","11397.49","3879.72","16628.71",3894,211721,25687,10365,"2024-04-11 16:02:08",217,153,{"Medal_ID":217,"Name":"Not Bluffing","Link":"osu-secret-bluffing.png","Frequency":5.44616e-6,"Count_Achieved_By":153},"93.66"]],"types":["integer","integer","string","string","string","string","string","integer","integer","integer","integer","integer","integer","string","integer","integer","integer","integer","integer","integer","string","string","string","string","string","string","string","integer","integer","integer","integer","string","integer","integer","array","string"]},"max":10000}}"#;

        let OsekaiInex {
            content:
                OsekaiRankingEntries {
                    data: CompactWrap::<OsekaiRankingEntry>(entries),
                },
        } = serde_json::from_str(JSON).unwrap();

        println!("{entries:#?}");
    }

    #[test]
    fn deserialize_rarity_ranking() {
        const JSON: &'static str = r#"{"_dev":"Compressed response. Remove 'compress' parameter for human-readable form.","success":true,"message":"ok","content":{"data":{"_t":true,"k":["Rank","Medal_ID","Name","Link","Description","Gamemode","Grouping","Instructions","Ordering","Frequency","Count_Achieved_By","Achieved"],"d":[[1,217,"Not Bluffing","osu-secret-bluffing.png","Did that with my eyes closed.","osu","Hush-Hush (Expert)","<i>It's not arrogance if you can live up to it.<\/i>",3,5.44616e-6,153,false],[2,342,"Pioneer","all-secret-pioneer.png","Call sign: Cedar",null,"Hush-Hush (Expert)","<i>Liftoff schedule at subunit precision.<\/i>",3,5.73093e-6,161,false],[3,245,"Unfathomable","osu-skill-fc-10.png","You have no equal.","osu","Skill & Dedication","<i>Cement your place among legends, and FC any 10 star map.<\/i>",5,6.0157e-6,169,false],[4,192,"Skylord","osu-secret-skylord.png","Never miss a wingbeat.","osu","Hush-Hush (Expert)","<i>Flight among the sky requires nothing but perfection.<\/i>",3,6.40725e-6,180,false],[5,344,"Stargazer","taiko-secret-stargazer.png","Three for the almanac.","taiko","Hush-Hush (Expert)","<i>Return later.<\/i>",3,1.85148e-5,257,false],[6,324,"Anabasis","taiko-secret-anabasis.png","And what an adventure you had.","taiko","Hush-Hush (Expert)","<i>Life is but a journey of the senses.<\/i>",3,1.86589e-5,259,false],[7,291,"30,000,000 Drum Hits","taiko-hits-30000000.png","Your rhythm, eternal.","taiko","Skill & Dedication",null,3,1.87309e-5,260,false],[8,332,"Hybrid Hyperion","all-secret-hybrid.png","Manifold conqueror.",null,"Hush-Hush (Expert)","<i>You proteans should show off more.<\/i>",3,1.06076e-5,298,false],[9,118,"Behind The Veil","mania-skill-fc-8.png","Supernatural!","mania","Skill & Dedication",null,5,3.02008e-5,428,false],[10,110,"Dashing Scarlet","fruits-skill-fc-8.png","Speed beyond mortal reckoning.","catch","Skill & Dedication",null,5,3.76056e-5,461,false],[11,347,"The Strongest Ice Fairy","osu-secret-icefairy.png","Extreme Sign \"Perfect Metal\"","osu","Hush-Hush (Expert)","<b>osu!(lazer) only<\/b><br><i>Unfazed by eidetic patterns.<\/i>",3,1.77267e-5,498,false],[12,281,"Beast Mode","osu-secret-beastmode.png","Unleash the animal within!","osu","Hush-Hush (Expert)","<i>Go absolutely feral.<\/i>",3,1.81539e-5,510,false],[13,267,"Mappers' Guild Pack IX","all-packs-mappersguild-09.png","This is no subtle change.",null,"Beatmap Challenge Packs","<i>Play all of the Mappers' Guild IX pack, which require no difficulty reduction mods when submitting a score.<\/i>",6,1.88658e-5,530,false],[14,333,"Divination Break","all-secret-divinationbreak.png","So cold...",null,"Hush-Hush (Expert)","<i>The frozen thread shattered.<\/i>",3,1.90794e-5,536,false],[15,195,"Mirage","all-secret-mirage.png","The horizon goes on forever, and ever, and ever...",null,"Hush-Hush (Expert)","<i>The light is far harder than it seems.<\/i>",3,2.52019e-5,708,false],[16,345,"Candescence","osu-secret-candescence.png","Guided by blazing resolve.","osu","Hush-Hush (Expert)","<b>osu!(lazer) only<\/b><br><i>Allow no shortcuts.<\/i>",3,2.60918e-5,733,false],[17,199,"Aeon","all-secret-aeon.png","In the mire of thawing time, memory shall be your guide.",null,"Hush-Hush","<i>When time runs slow and sight fails you, how will you succeed?<\/i>",2,2.84055e-5,798,false],[18,343,"Overcooked?","osu-secret-overcooked.png","Come and watch me!","osu","Hush-Hush (Expert)","<i>Internet lemonade!<\/i>",3,3.0826e-5,866,false],[19,266,"Mappers' Guild Pack VIII","all-packs-mappersguild-08.png","Succeed with a chorus of voices.",null,"Beatmap Challenge Packs","<i>Play all of the Mappers' Guild VIII pack, which require no difficulty reduction mods when submitting a score.<\/i>",6,3.15023e-5,885,false],[20,351,"Weather Reverie","catch-secret-weatherreverie.png","Up and above the world...","catch","Hush-Hush","<b>osu!(lazer) only<\/b><br><i>...Like a fruit tray in the sky.<\/i>",2,7.44771e-5,913,false],[21,45,"Jack of All Trades","all-secret-jack.png","Good at everything.",null,"Hush-Hush",null,2,3.29973e-5,927,false],[22,243,"Chosen","osu-skill-fc-9.png","Reign among the Prometheans, where you belong.","osu","Skill & Dedication","<i>Triumph over one of the hardest beatmaps ever made, and FC a 9 star map.<\/i>",5,3.58094e-5,1006,false],[23,323,"Internment","osu-secret-internment.png","Stained in the hues of madness, they rage within a four-walled jail.","osu","Hush-Hush (Expert)","<a href=\"https:\/\/assets.ppy.sh\/medals\/internment_hint.jpg\"><i>Bask in the shade of an ailing mind.<\/i><\/a>",3,3.64145e-5,1023,false],[24,278,"Sanguine","osu-secret-sanguine.png","Timeless thorns still draw blood.","osu","Hush-Hush","<i>Return to the past, when everything was simpler.<\/i>",2,3.64501e-5,1024,false],[25,305,"Astronomic","all-secret-astronomic.png","Precision machinery.",null,"Hush-Hush (Expert)","<i>Celestial coordinates: 284-905.<\/i>",3,3.78384e-5,1063,false],[26,350,"Project Loved: Best of 2024","loved-seasonal-2024.png","In the freezing midwinter, I can still barely hear your voice...",null,"Beatmap Packs","<i>Play any of the Project Loved: Best of 2024 beatmap packs.<\/i>",6,3.9013e-5,1096,false],[27,109,"Supersonic","fruits-skill-fc-7.png","Faster than is reasonably necessary.","catch","Skill & Dedication",null,5,9.37286e-5,1149,false],[28,298,"Dark Familiarity","all-secret-dark-familiarity.png","No mistakes, no witnesses.","osu","Hush-Hush (Expert)","<i>Don't second guess yourself.<\/i>",3,4.54203e-5,1276,false],[29,327,"Literal","taiko-secret-literal.png","The instructions were very clear.","taiko","Hush-Hush (Expert)","<i>...but probably not optimal.<\/i>",3,9.30062e-5,1291,false],[30,294,"Ariabl'eyeS Pack","all-packs-ariableyes.png","Command the mercurial skies.",null,"Beatmap Challenge Packs","<i>Play all of the Ariabl'eyes beatmap pack, which requires no difficulty reduction mods.<\/i>",6,4.77696e-5,1342,false],[31,292,"Catch 20,000,000 fruits","fruits-hits-20000000.png","Nothing left behind.","catch","Skill & Dedication",null,3,0.000110125,1350,false],[32,277,"In Memoriam","osu-secret-inmemoriam.png","In loving memory of your sanity, long forgotten.","osu","Hush-Hush","<i>Conquer a test of the most frustrating combination of mods imaginable.<\/i>",2,4.88375e-5,1372,false],[33,307,"Iron Will","all-secret-iron-will.png","A legacy of broken bonds, remade.","osu","Hush-Hush (Expert)","<i>Step forth, and master three years of titan-ending tests.<\/i>",3,4.93358e-5,1386,false],[34,86,"Lord of the Catch","fruits-skill-pass-8.png","Your kingdom kneels before you.","catch","Skill & Dedication",null,4,0.00011404,1398,false],[35,352,"Up To Eleven","osu-secret-uptoeleven.png","Crank it up!","osu","Hush-Hush (Expert)","<b>osu!(lazer) only<\/b><br><i>Break limits.<\/i>",3,5.09376e-5,1431,false],[36,280,"Final Boss","osu-secret-finalboss.png","Game over.","osu","Hush-Hush (Expert)","<i>Let the credits roll.<\/i>",3,5.13292e-5,1442,false],[37,348,"Infectious Enthusiasm","all-secret-infectiousenthusiasm.png","You're a fungus.",null,"Hush-Hush","<b>osu!(lazer) only<\/b><br><i>:)<\/i>",2,5.18987e-5,1458,false],[38,357,"Waning Memory","osu-secret-waningmemory.png","An image reformed.","osu","Hush-Hush (Expert)","<i>Concealed reflection or genuine recollection.<\/i>",3,5.23615e-5,1471,false],[39,325,"Project Loved: Summer 2023","loved-seasonal-2023-summer.png","Keep those psychopathic bugs away from my flowers!",null,"Beatmap Packs","<i>Play any of the Project Loved: Summer 2023 beatmap packs.<\/i>",6,5.29666e-5,1488,false],[40,308,"USAO Pack","all-packs-usao.png","Now THAT is a showdown.",null,"Beatmap Challenge Packs","<i>Play all of the USAO beatmap pack, which requires no difficulty reduction mods.<\/i>",6,5.38565e-5,1513,false],[41,247,"Camellia II","all-packs-camellia-2.png","Exit the atmosphere.",null,"Beatmap Challenge Packs","<i>Play all of the Camellia Challenges pack, which requires no difficulty reduction mods active.<\/i>",6,5.39277e-5,1515,false],[42,286,"Lightless","all-secret-lightless.png","Better the devil you know.",null,"Hush-Hush (Expert)","<i>I CAN'T SEE A THING.<\/i>",3,5.51024e-5,1548,false],[43,146,"Is This Real Life?","osu-secret-supersuperhardhddt.png","You did NOT just pull that off.","osu","Hush-Hush (Expert)","<i>The absolute height of perfection.<\/i>",3,5.57431e-5,1566,false],[44,314,"Project Loved: Autumn 2022","loved-seasonal-2022-autumn.png","There's a god-ish bee in my damned salad.",null,"Beatmap Packs","<i>Play any of the Project Loved: Autumn 2022 beatmap packs.<\/i>",6,5.77365e-5,1622,false],[45,315,"Project Loved: Winter 2022","loved-seasonal-2022-winter.png","Chasing nightmares in square, alien and propane forms.",null,"Beatmap Packs","<i>Play any of the Project Loved: Winter 2022 beatmap packs.<\/i>",6,5.81636e-5,1634,false],[46,316,"Project Loved: Spring 2023","loved-seasonal-2023-spring.png","Phase one: turn all humans and their fickle souls into orange cordial.",null,"Beatmap Packs","<i>Play any of the Project Loved: Spring 2023 beatmap pack.<\/i>",6,5.95518e-5,1673,false],[47,265,"Mappers' Guild Pack VII","all-packs-mappersguild-07.png","A new set of vibrant challenges to overcome.",null,"Beatmap Challenge Packs","<i>Play all of the Mappers' Guild VII pack, which require no difficulty reduction mods when submitting a score.<\/i>",6,6.16876e-5,1733,false],[48,313,"Project Loved: Summer 2022","loved-seasonal-2022-summer.png","Alright, fine. You don't sound like dragonforce.",null,"Beatmap Packs","<i>Play any of the Project Loved: Summer 2022 beatmap packs.<\/i>",6,6.24351e-5,1754,false],[49,358,"Fading Reflection","osu-secret-fadingreflection.png","A falsehood adorned.","osu","Hush-Hush (Expert)","<i>Fabricated memory or unveiled identity.<\/i>",3,6.42861e-5,1806,false],[50,219,"Regicide","all-secret-regicide.png","A king no more.",null,"Hush-Hush (Expert)","<i>No throne lasts forever.<\/i>",3,6.68134e-5,1877,false]],"types":["integer","integer","string","string","string","string","string","string","integer","double","integer","boolean"]},"max":"347"}}"#;
        let OsekaiInex {
            content:
                OsekaiRankingEntries {
                    data: CompactWrap::<OsekaiRarityEntry>(entries),
                },
        } = serde_json::from_str(JSON).unwrap();

        println!("{entries:#?}");
    }
}
