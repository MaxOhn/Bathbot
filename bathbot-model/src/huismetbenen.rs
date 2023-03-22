use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter, Result as FmtResult},
};

use bathbot_util::osu::ModSelection;
use rosu_v2::prelude::{CountryCode, GameMods, RankStatus, Username};
use serde::{
    de::{Deserializer, Error, MapAccess, Unexpected, Visitor},
    Deserialize,
};
use time::{
    format_description::{
        modifier::{Day, Month, Padding, Year},
        Component, FormatItem,
    },
    Date, OffsetDateTime, PrimitiveDateTime,
};
use twilight_interactions::command::{CommandOption, CreateOption};

use super::deser;

#[derive(Debug)]
pub struct SnipeScoreParams {
    pub user_id: u32,
    pub country: CountryCode,
    pub page: u8,
    pub order: SnipePlayerListOrder,
    pub mods: Option<ModSelection>,
    pub descending: bool,
}

impl SnipeScoreParams {
    pub fn new(user_id: u32, country_code: impl AsRef<str>) -> Self {
        Self {
            user_id,
            country: country_code.as_ref().to_ascii_lowercase().into(),
            page: 1,
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

    pub fn page(&mut self, page: u8) {
        self.page = page;
    }
}

#[derive(Copy, Clone, CommandOption, CreateOption, Debug, Eq, PartialEq)]
pub enum SnipePlayerListOrder {
    #[option(name = "Accuracy", value = "acc")]
    Acc = 0,
    #[option(name = "Date", value = "date")]
    Date = 5,
    #[option(name = "Misses", value = "misses")]
    Misses = 3,
    #[option(name = "PP", value = "pp")]
    Pp = 4,
    #[option(name = "Stars", value = "stars")]
    Stars = 6,
}

impl Default for SnipePlayerListOrder {
    #[inline]
    fn default() -> Self {
        Self::Pp
    }
}

impl Display for SnipePlayerListOrder {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let name = match self {
            Self::Acc => "accuracy",
            Self::Misses => "count_miss",
            Self::Pp => "pp",
            Self::Date => "date_set",
            Self::Stars => "sr",
        };

        f.write_str(name)
    }
}

#[derive(Debug, Deserialize)]
pub struct SnipeCountryStatistics {
    #[serde(rename = "totalBeatmaps")]
    pub total_maps: usize,
    #[serde(rename = "unplayedBeatmaps")]
    pub unplayed_maps: usize,
    #[serde(rename = "topGain")]
    pub top_gain: Option<SnipeTopNationalDifference>,
    #[serde(rename = "topLoss")]
    pub top_loss: Option<SnipeTopNationalDifference>,
}

#[derive(Debug, Deserialize)]
pub struct SnipeTopNationalDifference {
    #[serde(rename = "most_recent_top_national")]
    pub top_national: Option<usize>,
    #[serde(rename = "name")]
    pub username: Username,
    #[serde(rename = "total_top_national_difference")]
    pub difference: i32,
}

#[derive(Debug, Deserialize)]
pub struct SnipePlayer {
    #[serde(rename = "name")]
    pub username: Username,
    pub user_id: u32,
    #[serde(rename = "average_pp")]
    pub avg_pp: f32,
    #[serde(rename = "average_accuracy", with = "deser::adjust_acc")]
    pub avg_acc: f32,
    #[serde(rename = "average_sr")]
    pub avg_stars: f32,
    #[serde(rename = "average_score")]
    pub avg_score: f32,
    #[serde(rename = "count")]
    pub count_first: u32,
    pub count_loved: u32,
    pub count_ranked: u32,
    #[serde(rename = "total_top_national_difference", default)]
    pub difference: i32,
    #[serde(rename = "mods_count", with = "mod_count", default)]
    pub count_mods: Option<Vec<(GameMods, u32)>>,
    #[serde(rename = "history_total_top_national", with = "history", default)]
    pub count_first_history: BTreeMap<Date, u32>,
    #[serde(rename = "sr_spread")]
    pub count_sr_spread: BTreeMap<i8, Option<u32>>,
    #[serde(rename = "oldest_date")]
    pub oldest_first: SnipePlayerOldest,
}

#[derive(Debug, Deserialize)]
pub struct SnipeCountryPlayer {
    #[serde(rename = "name")]
    pub username: Username,
    pub user_id: u32,
    #[serde(rename = "average_pp")]
    pub avg_pp: f32,
    #[serde(rename = "average_sr")]
    pub avg_sr: f32,
    #[serde(rename = "weighted_pp")]
    pub pp: f32,
    #[serde(rename = "count")]
    pub count_first: u32,
}

#[derive(Debug, Deserialize)]
pub struct SnipePlayerOldest {
    #[serde(with = "deser::negative_u32")]
    pub map_id: u32,
    pub map: String,
    #[serde(with = "datetime_mixture")]
    pub date: OffsetDateTime,
}

#[derive(Debug, Deserialize)]
pub struct SnipeRecent {
    pub uid: u32,
    #[serde(rename = "osu_score_id")]
    pub score_id: u64,
    pub map_id: u32,
    #[serde(rename = "player_id")]
    pub user_id: u32,
    pub country: rosu_v2::prelude::CountryCode,
    pub pp: Option<f32>,
    #[serde(rename = "sr")]
    pub stars: Option<f32>,
    #[serde(with = "deser::adjust_acc")]
    pub accuracy: f32,
    pub count_300: Option<u32>,
    pub count_100: Option<u32>,
    pub count_50: Option<u32>,
    pub count_miss: Option<u32>,
    #[serde(rename = "date_set", with = "deser::option_naive_datetime")]
    pub date: Option<OffsetDateTime>,
    pub mods: Option<GameMods>,
    pub max_combo: Option<u32>,
    pub ar: f32,
    pub cs: f32,
    pub od: f32,
    pub hp: f32,
    pub bpm: f32,
    pub artist: String,
    pub title: String,
    #[serde(rename = "diff_name")]
    pub version: String,
    #[serde(default, rename = "sniper_name")]
    pub sniper: Option<String>,
    pub sniper_id: u32,
    #[serde(default, rename = "sniped_name")]
    pub sniped: Option<String>,
    pub sniped_id: Option<u32>,
}

#[derive(Deserialize)]
pub struct SnipeScore {
    pub uid: u32,
    #[serde(rename = "osu_score_id")]
    pub score_id: u64,
    #[serde(rename = "player_id")]
    pub user_id: u32,
    pub username: Username,
    pub country: rosu_v2::prelude::CountryCode,
    pub score: u32,
    pub pp: Option<f32>,
    #[serde(rename = "sr")]
    pub stars: f32,
    #[serde(with = "deser::adjust_acc")]
    pub accuracy: f32,
    pub count_300: Option<u32>,
    pub count_100: Option<u32>,
    pub count_50: Option<u32>,
    pub count_miss: Option<u32>,
    #[serde(with = "deser::option_naive_datetime")]
    pub date_set: Option<OffsetDateTime>,
    pub mods: Option<GameMods>,
    pub max_combo: Option<u32>,
    pub ar: f32,
    pub hp: f32,
    pub cs: f32,
    pub od: f32,
    pub bpm: f32,
    pub is_global: bool,
    #[serde(rename = "beatmap")]
    pub map: SnipeBeatmap,
}

#[derive(Deserialize)]
pub struct SnipeBeatmap {
    pub map_id: u32,
    #[serde(rename = "set_id")]
    pub mapset_id: u32,
    pub artist: String,
    pub title: String,
    #[serde(rename = "diff_name")]
    pub version: String,
    #[serde(rename = "total_length")]
    pub seconds_total: u32,
    #[serde(with = "deser::naive_datetime")]
    pub date_ranked: OffsetDateTime,
    #[serde(rename = "count_normal")]
    pub count_circles: u32,
    #[serde(rename = "count_slider")]
    pub count_sliders: u32,
    #[serde(rename = "count_spinner")]
    pub count_spinners: u32,
    pub ranked_status: RankStatus,
    pub ar: f32,
    pub cs: f32,
    pub od: f32,
    pub hp: f32,
    pub bpm: f32,
    pub max_combo: u32,
}

mod mod_count {
    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<Vec<(GameMods, u32)>>, D::Error> {
        d.deserialize_map(SnipePlayerModVisitor).map(Some)
    }

    struct SnipePlayerModVisitor;

    impl<'de> Visitor<'de> for SnipePlayerModVisitor {
        type Value = Vec<(GameMods, u32)>;

        #[inline]
        fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
            f.write_str("a map")
        }

        fn visit_map<V: MapAccess<'de>>(self, mut map: V) -> Result<Self::Value, V::Error> {
            let mut mod_count = Vec::with_capacity(map.size_hint().unwrap_or(0));

            while let Some((mods, num)) = map.next_entry()? {
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
                    Error::invalid_value(Unexpected::Str(key), &"a date of the form `%F`")
                })?;

                history.insert(date, map.next_value()?);
            }

            Ok(history)
        }
    }
}

// Tries to deserialize various datetime formats
mod datetime_mixture {
    use bathbot_util::datetime::{NAIVE_DATETIME_FORMAT, OFFSET_FORMAT};
    use time::UtcOffset;

    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<OffsetDateTime, D::Error> {
        d.deserialize_str(DateTimeVisitor)
    }

    pub(super) struct DateTimeVisitor;

    impl<'de> Visitor<'de> for DateTimeVisitor {
        type Value = OffsetDateTime;

        fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
            f.write_str("a datetime string")
        }

        #[inline]
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            if v.len() < 19 {
                return Err(Error::custom(format!(
                    "string too short for a datetime: `{v}`"
                )));
            }

            let (prefix, suffix) = v.split_at(19);

            let primitive =
                PrimitiveDateTime::parse(prefix, NAIVE_DATETIME_FORMAT).map_err(Error::custom)?;

            let offset = if suffix.is_empty() || suffix == "Z" {
                UtcOffset::UTC
            } else {
                UtcOffset::parse(suffix, OFFSET_FORMAT).map_err(Error::custom)?
            };

            Ok(primitive.assume_offset(offset))
        }
    }
}
