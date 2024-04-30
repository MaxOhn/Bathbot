use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt::{Formatter, Result as FmtResult},
};

use bathbot_util::{osu::ModSelection, CowUtils};
use rkyv::{
    boxed::ArchivedBox, Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize,
};
use rosu_v2::prelude::{CountryCode, GameMode, GameMods, GameModsIntermode, Username};
use serde::{
    de::{
        value::StrDeserializer, Deserializer, Error as DeError, MapAccess, SeqAccess, Unexpected,
        Visitor,
    },
    Deserialize,
};
use serde_json::value::RawValue;
use time::{
    format_description::{
        modifier::{Day, Month, Padding, Year},
        Component, FormatItem,
    },
    Date, OffsetDateTime,
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
    pub total_maps: Option<u32>, // not available anymore with huismetbenen
    #[serde(rename = "unplayed_count")]
    pub unplayed_maps: u32,
    #[serde(rename = "most_gained_count")]
    pub most_gains_count: i32,
    #[serde(rename = "most_gained_player_id")]
    pub most_gains_user_id: u32,
    #[serde(rename = "most_gained_player_name")]
    pub most_gains_username: Username,
    #[serde(rename = "most_lost_count")]
    pub most_losses_count: i32,
    #[serde(rename = "most_lost_player_id")]
    pub most_losses_user_id: u32,
    #[serde(rename = "most_lost_player_name")]
    pub most_losses_username: Username,
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

#[derive(Debug, Deserialize)]
pub struct SnipePlayer {
    pub username: Username,
    pub user_id: u32,
    #[serde(rename = "average_pp")]
    pub avg_pp: f32,
    #[serde(rename = "average_acc", with = "deser::adjust_acc")]
    pub avg_acc: f32,
    #[serde(rename = "average_sr")]
    pub avg_stars: f32,
    #[serde(rename = "average_score")]
    pub avg_score: f32,
    #[serde(rename = "count_total")]
    pub count_first: u32,
    pub count_loved: u32,
    pub count_ranked: u32,
    #[serde(rename = "recent_history_difference")]
    pub difference: i32,
    #[serde(rename = "mods_count", with = "mods_count")]
    pub count_mods: ModsCount,
    #[serde(rename = "sr_spread", with = "sr_spread")]
    pub count_sr_spread: BTreeMap<i8, u32>,
    #[serde(rename = "oldest_date_map_id")]
    pub oldest_map_id: Option<u32>,
}

pub struct SnipePlayerHistory;

impl SnipePlayerHistory {
    pub fn deserialize(bytes: &[u8]) -> Result<BTreeMap<Date, u32>, serde_json::Error> {
        let mut d = serde_json::Deserializer::from_slice(bytes);

        history::deserialize(&mut d)
    }
}

#[derive(Debug, Deserialize)]
pub struct SnipeCountryPlayer {
    pub username: Username,
    pub user_id: u32,
    #[serde(rename = "average_pp")]
    pub avg_pp: Option<f32>,
    #[serde(rename = "average_sr")]
    pub avg_sr: f32,
    #[serde(rename = "weighted_pp")]
    pub pp: f32,
    #[serde(rename = "count_total")]
    pub count_first: u32,
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

mod mods_count {
    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<ModsCount, D::Error> {
        d.deserialize_map(SnipePlayerModVisitor)
    }

    struct SnipePlayerModVisitor;

    impl<'de> Visitor<'de> for SnipePlayerModVisitor {
        type Value = ModsCount;

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

mod sr_spread {
    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<i8, u32>, D::Error> {
        d.deserialize_map(StarRatingSpreadVisitor)
    }

    struct StarRatingSpreadVisitor;

    impl<'de> Visitor<'de> for StarRatingSpreadVisitor {
        type Value = BTreeMap<i8, u32>;

        fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
            f.write_str("a map")
        }

        fn visit_map<V: MapAccess<'de>>(self, mut map: V) -> Result<Self::Value, V::Error> {
            let mut sr_spread = BTreeMap::new();

            while let Some((sr, num)) = map.next_entry::<&str, Option<u32>>()? {
                let sr = sr
                    .parse()
                    .map_err(|_| DeError::invalid_value(Unexpected::Str(sr), &"an integer"))?;

                sr_spread.insert(sr, num.unwrap_or(0));
            }

            Ok(sr_spread)
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
        d.deserialize_seq(SnipePlayerHistoryVisitor)
    }

    struct SnipePlayerHistoryVisitor;

    impl<'de> Visitor<'de> for SnipePlayerHistoryVisitor {
        type Value = BTreeMap<Date, u32>;

        fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
            f.write_str("a sequence")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            #[derive(Deserialize)]
            struct Entry<'a> {
                history_date: &'a str,
                count_total: u32,
            }

            let mut history = BTreeMap::new();

            while let Some(entry) = seq.next_element::<Entry>()? {
                let date = Date::parse(entry.history_date, DATE_FORMAT).map_err(|_| {
                    DeError::invalid_value(
                        Unexpected::Str(entry.history_date),
                        &"a date of the form `%F`",
                    )
                })?;

                history.insert(date, entry.count_total);
            }

            Ok(history)
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
