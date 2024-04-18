use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt::{Formatter, Result as FmtResult},
    ops::ControlFlow,
};

use bathbot_util::{osu::ModSelection, CowUtils};
use rkyv::{
    boxed::ArchivedBox, Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize,
};
use rosu_v2::prelude::{CountryCode, GameMode, GameMods, GameModsIntermode, Username};
use serde::{
    de::{
        value::StrDeserializer, Deserializer, Error as DeError, IgnoredAny, MapAccess, SeqAccess,
        Unexpected, Visitor,
    },
    Deserialize,
};
use serde_json::{value::RawValue, Deserializer as JsonDeserializer, Error as JsonError};
use time::{
    format_description::{
        modifier::{Day, Month, Padding, Year},
        Component, FormatItem,
    },
    Date, OffsetDateTime, PrimitiveDateTime,
};
use twilight_interactions::command::{CommandOption, CreateOption};

use super::deser;
use crate::KittenRoleplayCountries;

pub struct SnipeScoreParams {
    pub user_id: u32,
    pub country: CountryCode,
    pub mode: GameMode,
    pub page: u32,
    pub limit: Option<u8>,
    pub order: SnipePlayerListOrder,
    pub mods: Option<ModSelection>,
    pub descending: bool,
}

impl SnipeScoreParams {
    pub fn new(user_id: u32, country_code: &str, mode: GameMode) -> Self {
        Self {
            user_id,
            country: country_code.to_ascii_lowercase().into(),
            mode,
            page: 1,
            limit: None,
            order: SnipePlayerListOrder::Pp,
            mods: None,
            descending: true,
        }
    }

    pub fn order(mut self, order: SnipePlayerListOrder) -> Self {
        self.order = order;

        self
    }

    pub fn descending(mut self, descending: bool) -> Self {
        self.descending = descending;

        self
    }

    pub fn mods(mut self, selection: Option<ModSelection>) -> Self {
        self.mods = selection;

        self
    }

    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit);

        self
    }

    pub fn page(&mut self, page: u32) {
        self.page = page;
    }
}

#[derive(Copy, Clone, CommandOption, CreateOption, Default, Eq, PartialEq)]
pub enum SnipeCountryListOrder {
    #[default]
    #[option(name = "Count", value = "count")]
    Count,
    #[option(name = "Average PP", value = "avg_pp")]
    AvgPp,
    #[option(name = "Average Stars", value = "avg_stars")]
    AvgStars,
    #[option(name = "Weighted PP", value = "weighted_pp")]
    WeightedPp,
}

impl SnipeCountryListOrder {
    pub fn as_huismetbenen_str(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::AvgPp => "pp/average",
            Self::AvgStars => "sr/average",
            Self::WeightedPp => "pp/weighted",
        }
    }

    pub fn as_kittenroleplay_str(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::AvgPp => "average_pp",
            Self::AvgStars => "average_stars",
            Self::WeightedPp => "weighted_pp",
        }
    }
}

#[derive(Copy, Clone, CommandOption, CreateOption, Debug, Default, Eq, PartialEq)]
pub enum SnipePlayerListOrder {
    #[option(name = "Accuracy", value = "acc")]
    Acc = 0,
    #[option(name = "Date", value = "date")]
    Date = 5,
    #[option(name = "Misses", value = "misses")]
    Misses = 3,
    #[default]
    #[option(name = "PP", value = "pp")]
    Pp = 4,
    #[option(name = "Stars", value = "stars")]
    Stars = 6,
}

impl SnipePlayerListOrder {
    pub fn as_huismetbenen_str(self) -> &'static str {
        match self {
            Self::Acc => "accuracy",
            Self::Date => "date_set",
            Self::Misses => "count_miss",
            Self::Pp => "pp",
            Self::Stars => "sr",
        }
    }

    pub fn as_kittenroleplay_str(self) -> &'static str {
        match self {
            Self::Acc => "accuracy",
            Self::Date => "created_at",
            Self::Misses => "count_miss",
            Self::Pp => "pp",
            Self::Stars => "stars",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SnipeCountryStatistics {
    #[serde(rename = "totalBeatmaps")]
    pub total_maps: u32,
    #[serde(rename = "unplayedBeatmaps")]
    pub unplayed_maps: u32,
    #[serde(rename = "topGain")]
    pub top_gain: Option<SnipeTopNationalDifference>,
    #[serde(rename = "topLoss")]
    pub top_loss: Option<SnipeTopNationalDifference>,
}

#[derive(Debug, Deserialize)]
pub struct SnipeTopNationalDifference {
    #[serde(rename = "most_recent_top_national")]
    pub top_national: Option<u32>,
    #[serde(rename = "name")]
    pub username: Username,
    #[serde(rename = "total_top_national_difference")]
    pub difference: i32,
}

pub type ModsCount = Vec<(Box<str>, u32)>;

// We'll implement Deserialize manually because a certain value for a field
// should imply that this whole type is null.
// Specifically, `oldest_first::date` has a weird default value if a user
// has had national #1s but does not currently.
#[derive(Debug)]
pub struct SnipePlayer {
    pub username: Username,
    pub user_id: u32,
    pub avg_pp: f32,
    pub avg_acc: f32,
    pub avg_stars: f32,
    pub avg_score: f32,
    pub count_first: u32,
    pub count_loved: u32,
    pub count_ranked: u32,
    pub difference: i32,
    pub count_mods: Option<ModsCount>,
    pub count_first_history: BTreeMap<Date, u32>,
    pub count_sr_spread: BTreeMap<i8, Option<u32>>,
    pub oldest_first: SnipePlayerOldest,
}

impl SnipePlayer {
    pub fn deserialize(bytes: &[u8]) -> Result<Option<Self>, JsonError> {
        struct SnipePlayerVisitor;

        impl<'de> Visitor<'de> for SnipePlayerVisitor {
            type Value = Option<SnipePlayer>;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("a SnipePlayer")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                struct AvgAcc(f32);

                impl<'de> Deserialize<'de> for AvgAcc {
                    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                        deser::adjust_acc::deserialize(d).map(Self)
                    }
                }

                struct CountMods(Option<ModsCount>);

                impl<'de> Deserialize<'de> for CountMods {
                    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                        mod_count::deserialize(d).map(Self)
                    }
                }

                struct CountFirstHistory(BTreeMap<Date, u32>);

                impl<'de> Deserialize<'de> for CountFirstHistory {
                    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                        history::deserialize(d).map(Self)
                    }
                }

                struct OldestFirst(Option<SnipePlayerOldest>);

                impl<'de> Deserialize<'de> for OldestFirst {
                    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                        struct OldestFirstVisitor;

                        impl<'de> Visitor<'de> for OldestFirstVisitor {
                            type Value = Option<SnipePlayerOldest>;

                            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                                f.write_str("a SnipePlayerOldest")
                            }

                            fn visit_map<A: MapAccess<'de>>(
                                self,
                                mut map: A,
                            ) -> Result<Self::Value, A::Error> {
                                struct MapId(u32);

                                impl<'de> Deserialize<'de> for MapId {
                                    fn deserialize<D: Deserializer<'de>>(
                                        d: D,
                                    ) -> Result<Self, D::Error>
                                    {
                                        deser::negative_u32::deserialize(d).map(Self)
                                    }
                                }

                                struct Date(Option<OffsetDateTime>);

                                impl<'de> Deserialize<'de> for Date {
                                    fn deserialize<D: Deserializer<'de>>(
                                        d: D,
                                    ) -> Result<Self, D::Error>
                                    {
                                        oldest_datetime::deserialize(d).map(Self)
                                    }
                                }

                                let mut map_id: Option<u32> = None;
                                let mut map_: Option<Box<str>> = None;
                                let mut date: Option<ControlFlow<(), OffsetDateTime>> = None;

                                while let Some(key) = map.next_key()? {
                                    match key {
                                        "map_id" => {
                                            let MapId(id) = map.next_value()?;
                                            map_id = Some(id);
                                        }
                                        "map" => map_ = Some(map.next_value()?),
                                        "date" => {
                                            let Date(opt) = map.next_value()?;

                                            date = Some(opt.map_or(
                                                ControlFlow::Break(()),
                                                ControlFlow::Continue,
                                            ));
                                        }
                                        _ => {
                                            let _: IgnoredAny = map.next_value()?;
                                        }
                                    }
                                }

                                let date = match date {
                                    Some(ControlFlow::Continue(date)) => date,
                                    Some(ControlFlow::Break(_)) => return Ok(None),
                                    None => return Err(DeError::missing_field("date")),
                                };

                                let map_id =
                                    map_id.ok_or_else(|| DeError::missing_field("map_id"))?;
                                let map = map_.ok_or_else(|| DeError::missing_field("map"))?;

                                let oldest = SnipePlayerOldest { map_id, map, date };

                                Ok(Some(oldest))
                            }
                        }

                        d.deserialize_map(OldestFirstVisitor).map(Self)
                    }
                }

                let mut username: Option<Username> = None;
                let mut user_id: Option<u32> = None;
                let mut avg_pp: Option<f32> = None;
                let mut avg_acc: Option<f32> = None;
                let mut avg_stars: Option<f32> = None;
                let mut avg_score: Option<f32> = None;
                let mut count_first: Option<u32> = None;
                let mut count_loved: Option<u32> = None;
                let mut count_ranked: Option<u32> = None;
                let mut difference: Option<i32> = None;
                let mut count_mods: Option<ModsCount> = None;
                let mut count_first_history: Option<BTreeMap<Date, u32>> = None;
                let mut count_sr_spread: Option<BTreeMap<i8, Option<u32>>> = None;
                let mut oldest_first: Option<ControlFlow<(), SnipePlayerOldest>> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "name" => username = Some(map.next_value()?),
                        "user_id" => user_id = Some(map.next_value()?),
                        "average_pp" => avg_pp = Some(map.next_value()?),
                        "average_accuracy" => {
                            let AvgAcc(acc) = map.next_value()?;
                            avg_acc = Some(acc);
                        }
                        "average_sr" => avg_stars = Some(map.next_value()?),
                        "average_score" => avg_score = Some(map.next_value()?),
                        "count" => count_first = Some(map.next_value()?),
                        "count_loved" => count_loved = Some(map.next_value()?),
                        "count_ranked" => count_ranked = Some(map.next_value()?),
                        "total_top_national_difference" => difference = Some(map.next_value()?),
                        "mods_count" => {
                            let CountMods(counts) = map.next_value()?;
                            count_mods = counts;
                        }
                        "history_total_top_national" => {
                            let CountFirstHistory(history) = map.next_value()?;
                            count_first_history = Some(history);
                        }
                        "sr_spread" => count_sr_spread = Some(map.next_value()?),
                        "oldest_date" => {
                            let OldestFirst(opt) = map.next_value()?;

                            oldest_first =
                                Some(opt.map_or(ControlFlow::Break(()), ControlFlow::Continue));
                        }
                        _ => {
                            let _: IgnoredAny = map.next_value()?;
                        }
                    }
                }

                if username.is_none()
                    && user_id.is_none()
                    && avg_pp.is_none()
                    && avg_acc.is_none()
                    && avg_stars.is_none()
                    && avg_score.is_none()
                    && count_first.is_none()
                    && count_loved.is_none()
                    && count_ranked.is_none()
                    && difference.is_none()
                    && count_mods.is_none()
                    && count_first_history.is_none()
                    && count_sr_spread.is_none()
                    && oldest_first.is_none()
                {
                    return Ok(None);
                }

                let oldest_first = match oldest_first {
                    Some(ControlFlow::Continue(oldest)) => oldest,
                    Some(ControlFlow::Break(_)) => return Ok(None),
                    None => return Err(DeError::missing_field("oldest_date")),
                };

                let username = username.ok_or_else(|| DeError::missing_field("name"))?;
                let user_id = user_id.ok_or_else(|| DeError::missing_field("user_id"))?;
                let avg_pp = avg_pp.ok_or_else(|| DeError::missing_field("average_pp"))?;
                let avg_acc = avg_acc.ok_or_else(|| DeError::missing_field("average_accuracy"))?;
                let avg_stars = avg_stars.ok_or_else(|| DeError::missing_field("average_sr"))?;
                let avg_score = avg_score.ok_or_else(|| DeError::missing_field("average_score"))?;
                let count_first = count_first.ok_or_else(|| DeError::missing_field("count"))?;
                let count_loved =
                    count_loved.ok_or_else(|| DeError::missing_field("count_loved"))?;
                let count_ranked =
                    count_ranked.ok_or_else(|| DeError::missing_field("count_ranked"))?;
                let difference = difference.unwrap_or_default();
                let count_first_history = count_first_history.unwrap_or_default();
                let count_sr_spread =
                    count_sr_spread.ok_or_else(|| DeError::missing_field("sr_spread"))?;

                let snipe_player = SnipePlayer {
                    username,
                    user_id,
                    avg_pp,
                    avg_acc,
                    avg_stars,
                    avg_score,
                    count_first,
                    count_loved,
                    count_ranked,
                    difference,
                    count_mods,
                    count_first_history,
                    count_sr_spread,
                    oldest_first,
                };

                Ok(Some(snipe_player))
            }
        }

        JsonDeserializer::from_slice(bytes).deserialize_map(SnipePlayerVisitor)
    }
}

#[derive(Debug, Deserialize)]
pub struct SnipeCountryPlayer {
    #[serde(rename = "name")]
    pub username: Username,
    pub user_id: u32,
    #[serde(rename = "average_pp")]
    pub avg_pp: Option<f32>,
    #[serde(rename = "average_sr")]
    pub avg_sr: f32,
    #[serde(rename = "weighted_pp")]
    pub pp: f32,
    #[serde(rename = "count")]
    pub count_first: u32,
}

#[derive(Debug)]
pub struct SnipePlayerOldest {
    pub map_id: u32,
    pub map: Box<str>,
    pub date: OffsetDateTime,
}

#[derive(Debug)]
pub struct SnipeRecent {
    pub map_id: u32,
    pub user_id: u32,
    pub pp: Option<f32>,
    pub stars: Option<f32>,
    pub accuracy: f32,
    pub date: Option<OffsetDateTime>,
    pub mods: Option<GameMods>,
    pub max_combo: Option<u32>,
    pub artist: Box<str>,
    pub title: Box<str>,
    pub version: Box<str>,
    pub sniper: Option<Username>,
    pub sniper_id: u32,
    pub sniped: Option<Username>,
    pub sniped_id: Option<u32>,
}

impl<'de> Deserialize<'de> for SnipeRecent {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        pub struct SnipeRecentInner<'mods> {
            map_id: u32,
            #[serde(rename = "player_id")]
            user_id: u32,
            pp: Option<f32>,
            #[serde(rename = "sr")]
            stars: Option<f32>,
            #[serde(with = "deser::adjust_acc")]
            accuracy: f32,
            #[serde(rename = "date_set", with = "deser::option_naive_datetime")]
            date: Option<OffsetDateTime>,
            #[serde(borrow)]
            mods: Option<&'mods RawValue>,
            max_combo: Option<u32>,
            artist: Box<str>,
            title: Box<str>,
            #[serde(rename = "diff_name")]
            version: Box<str>,
            #[serde(default, rename = "sniper_name")]
            sniper: Option<Username>,
            sniper_id: u32,
            #[serde(default, rename = "sniped_name")]
            sniped: Option<Username>,
            sniped_id: Option<u32>,
        }

        let inner = <SnipeRecentInner as Deserialize>::deserialize(d)?;

        let mods = match inner.mods {
            Some(raw) => StrDeserializer::<D::Error>::new(raw.get().trim_matches('"'))
                .deserialize_str(SnipeModsVisitor)
                .map(Some)
                .map_err(DeError::custom)?,
            None => None,
        };

        Ok(SnipeRecent {
            map_id: inner.map_id,
            user_id: inner.user_id,
            pp: inner.pp,
            stars: inner.stars,
            accuracy: inner.accuracy,
            date: inner.date,
            mods,
            max_combo: inner.max_combo,
            artist: inner.artist,
            title: inner.title,
            version: inner.version,
            sniper: inner.sniper,
            sniper_id: inner.sniper_id,
            sniped: inner.sniped,
            sniped_id: inner.sniped_id,
        })
    }
}

struct SnipeModsVisitor;

impl<'de> Visitor<'de> for SnipeModsVisitor {
    type Value = GameMods;

    fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("GameMods")
    }

    fn visit_str<E: DeError>(self, v: &str) -> Result<Self::Value, E> {
        if v == "nomod" {
            return Ok(GameMods::new());
        }

        let intermode = match v.parse::<GameModsIntermode>() {
            Ok(mods) => mods,
            Err(_) => {
                let expected = "a valid combination of mod acronyms";

                return Err(DeError::invalid_value(Unexpected::Str(v), &expected));
            }
        };

        intermode
            .with_mode(GameMode::Osu)
            .ok_or_else(|| DeError::custom("invalid mods for mode"))
    }
}

pub struct SnipeScore {
    pub score: u32,
    pub pp: Option<f32>,
    pub stars: f32,
    pub accuracy: f32,
    pub count_miss: Option<u32>,
    pub date_set: Option<OffsetDateTime>,
    pub mods: Option<GameMods>,
    pub max_combo: Option<u32>,
    pub map_id: u32,
}

impl<'de> Deserialize<'de> for SnipeScore {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct SnipeBeatmap {
            map_id: u32,
        }

        #[derive(Deserialize)]
        struct SnipeScoreInner<'mods> {
            score: u32,
            pp: Option<f32>,
            #[serde(rename = "sr")]
            stars: f32,
            #[serde(with = "deser::adjust_acc")]
            accuracy: f32,
            count_miss: Option<u32>,
            #[serde(with = "deser::option_naive_datetime")]
            date_set: Option<OffsetDateTime>,
            #[serde(borrow)]
            mods: Option<&'mods RawValue>,
            max_combo: Option<u32>,
            #[serde(rename = "beatmap")]
            map: SnipeBeatmap,
        }

        let inner = <SnipeScoreInner<'_> as Deserialize>::deserialize(d)?;

        let mods = match inner.mods {
            Some(raw) => StrDeserializer::<D::Error>::new(raw.get().trim_matches('"'))
                .deserialize_str(SnipeModsVisitor)
                .map(Some)
                .map_err(DeError::custom)?,
            None => None,
        };

        Ok(SnipeScore {
            score: inner.score,
            pp: inner.pp,
            stars: inner.stars,
            accuracy: inner.accuracy,
            count_miss: inner.count_miss,
            date_set: inner.date_set,
            mods,
            max_combo: inner.max_combo,
            map_id: inner.map.map_id,
        })
    }
}

#[derive(Debug, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct SnipeCountries {
    country_codes: Box<[Box<str>]>,
}

impl SnipeCountries {
    pub fn sort(&mut self) {
        self.country_codes.sort_unstable();
    }
}

impl From<KittenRoleplayCountries> for SnipeCountries {
    fn from(countries: KittenRoleplayCountries) -> Self {
        let country_codes = countries
            .full
            .into_iter()
            .chain(countries.partial)
            .map(|country| country.code.into_boxed_str())
            .collect();

        Self { country_codes }
    }
}

impl SnipeCountries {
    pub fn contains(&self, country_code: &str) -> bool {
        self.country_codes
            .binary_search_by_key(&country_code, Box::as_ref)
            .is_ok()
    }
}

impl ArchivedSnipeCountries {
    pub fn contains(&self, country_code: &str) -> bool {
        self.country_codes
            .binary_search_by_key(&country_code, ArchivedBox::as_ref)
            .is_ok()
    }
}

impl<'de> Deserialize<'de> for SnipeCountries {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct SnipeCountriesVisitor;

        impl<'de> Visitor<'de> for SnipeCountriesVisitor {
            type Value = SnipeCountries;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("a sequence of snipe countries")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                #[derive(Deserialize)]
                struct SnipeCountry {
                    #[serde(deserialize_with = "snipe_country_code")]
                    country_code: Box<str>,
                }

                fn snipe_country_code<'de, D>(d: D) -> Result<Box<str>, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    match <&str as Deserialize>::deserialize(d)?.cow_to_ascii_uppercase() {
                        Cow::Borrowed(country_code) => Ok(Box::from(country_code)),
                        Cow::Owned(country_code) => Ok(country_code.into_boxed_str()),
                    }
                }

                let mut country_codes = Vec::with_capacity(seq.size_hint().unwrap_or(64));

                while let Some(SnipeCountry { country_code }) = seq.next_element()? {
                    country_codes.push(country_code);
                }

                country_codes.sort_unstable();

                Ok(SnipeCountries {
                    country_codes: country_codes.into_boxed_slice(),
                })
            }
        }

        d.deserialize_seq(SnipeCountriesVisitor)
    }
}

mod mod_count {
    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<ModsCount>, D::Error> {
        d.deserialize_map(SnipePlayerModVisitor).map(Some)
    }

    struct SnipePlayerModVisitor;

    impl<'de> Visitor<'de> for SnipePlayerModVisitor {
        type Value = ModsCount;

        #[inline]
        fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
            f.write_str("a map")
        }

        fn visit_map<V: MapAccess<'de>>(self, mut map: V) -> Result<Self::Value, V::Error> {
            let mut mod_count = Vec::with_capacity(map.size_hint().unwrap_or(0));

            while let Some((mut mods, num)) = map.next_entry::<Box<str>, _>()? {
                if mods.as_ref() == "nomod" {
                    mods = Box::from("NM");
                }

                mod_count.push((mods, num));
            }

            Ok(mod_count)
        }
    }
}

mod history {
    use super::*;

    const DATE_FORMAT: &[FormatItem<'_>] = &[
        FormatItem::Component(Component::Year(Year::default())),
        FormatItem::Literal(b"-"),
        FormatItem::Component(Component::Month({
            let mut month = Month::default();
            month.padding = Padding::None;

            month
        })),
        FormatItem::Literal(b"-"),
        FormatItem::Component(Component::Day({
            let mut day = Day::default();
            day.padding = Padding::None;

            day
        })),
    ];

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<Date, u32>, D::Error> {
        d.deserialize_map(SnipePlayerHistoryVisitor)
    }

    struct SnipePlayerHistoryVisitor;

    impl<'de> Visitor<'de> for SnipePlayerHistoryVisitor {
        type Value = BTreeMap<Date, u32>;

        fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
            f.write_str("a map")
        }

        fn visit_map<V: MapAccess<'de>>(self, mut map: V) -> Result<Self::Value, V::Error> {
            let mut history = BTreeMap::new();

            while let Some(key) = map.next_key()? {
                let date = Date::parse(key, DATE_FORMAT).map_err(|_| {
                    DeError::invalid_value(Unexpected::Str(key), &"a date of the form `%F`")
                })?;

                history.insert(date, map.next_value()?);
            }

            Ok(history)
        }
    }
}

// Tries to deserialize various datetime formats
mod oldest_datetime {
    use bathbot_util::datetime::{NAIVE_DATETIME_FORMAT, OFFSET_FORMAT};
    use time::UtcOffset;

    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<OffsetDateTime>, D::Error> {
        d.deserialize_str(DateTimeVisitor)
    }

    pub(super) struct DateTimeVisitor;

    impl<'de> Visitor<'de> for DateTimeVisitor {
        type Value = Option<OffsetDateTime>;

        fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
            f.write_str("a datetime string")
        }

        fn visit_str<E: DeError>(self, v: &str) -> Result<Self::Value, E> {
            const NULL_PREFIX: &str = "Fri Jan 01 9999";

            if v.len() < 19 {
                return Err(DeError::custom(format!(
                    "string too short for a datetime: `{v}`"
                )));
            } else if v.starts_with(NULL_PREFIX) {
                return Ok(None);
            }

            let (prefix, suffix) = v.split_at(19);

            let primitive =
                PrimitiveDateTime::parse(prefix, NAIVE_DATETIME_FORMAT).map_err(DeError::custom)?;

            let offset = if suffix.is_empty() || suffix == "Z" {
                UtcOffset::UTC
            } else {
                UtcOffset::parse(suffix, OFFSET_FORMAT).map_err(DeError::custom)?
            };

            Ok(Some(primitive.assume_offset(offset)))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SnipedWeek {
    #[serde(rename = "after", with = "deser::datetime_rfc2822")]
    pub from: OffsetDateTime,
    #[serde(rename = "before", with = "deser::datetime_rfc2822")]
    pub until: OffsetDateTime,
    #[serde(rename = "top", deserialize_with = "deser_sniped_players")]
    pub players: Vec<SnipedPlayer>,
    pub total: u32,
    pub unique: u32,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct SnipedPlayer {
    pub username: Username,
    pub count: u32,
}

// Custom deserialization to specify a capacity of 10
fn deser_sniped_players<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<SnipedPlayer>, D::Error> {
    struct SnipedPlayersVisitor;

    impl<'de> Visitor<'de> for SnipedPlayersVisitor {
        type Value = Vec<SnipedPlayer>;

        fn expecting(&self, f: &mut Formatter) -> FmtResult {
            f.write_str("a sequence of SnipedPlayer")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut players = Vec::with_capacity(10);

            while let Some(player) = seq.next_element()? {
                players.push(player);
            }

            Ok(players)
        }
    }

    d.deserialize_seq(SnipedPlayersVisitor)
}
