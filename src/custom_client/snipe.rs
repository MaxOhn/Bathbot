use std::{collections::BTreeMap, fmt};

use rosu_v2::{
    model::{GameMode, GameMods},
    prelude::Username,
};
use serde::{
    de::{Deserializer, Error, IgnoredAny, MapAccess, Unexpected, Visitor},
    Deserialize,
};
use time::{
    format_description::{
        modifier::{Day, Month, Padding, Year},
        Component, FormatItem,
    },
    Date, OffsetDateTime, PrimitiveDateTime,
};

use crate::{
    commands::osu::SnipePlayerListOrder,
    util::{datetime::NAIVE_DATETIME_FORMAT, osu::ModSelection, CountryCode},
};

use super::deser;

#[derive(Debug)]
pub struct SnipeScoreParams {
    pub user_id: u32,
    pub country: CountryCode,
    pub page: u8,
    pub mode: GameMode,
    pub order: SnipePlayerListOrder,
    pub mods: Option<ModSelection>,
    pub descending: bool,
}

impl SnipeScoreParams {
    pub fn new(user_id: u32, country_code: impl AsRef<str>) -> Self {
        Self {
            user_id,
            country: country_code.as_ref().to_ascii_lowercase().into(),
            page: 0,
            mode: GameMode::Osu,
            order: SnipePlayerListOrder::Pp,
            mods: None,
            descending: true,
        }
    }

    #[allow(dead_code)]
    pub fn mode(mut self, mode: GameMode) -> Self {
        self.mode = mode;

        self
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
    pub count_sr_spread: BTreeMap<u8, u32>,
    #[serde(rename = "oldest_date")]
    pub oldest_first: Option<SnipePlayerOldest>,
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
    #[serde(rename = "map_id", with = "deser::negative_u32")]
    pub beatmap_id: u32,
    pub map: String,
    #[serde(with = "option_datetime")]
    pub date: Option<OffsetDateTime>,
}

#[derive(Debug, Deserialize)]
pub struct SnipeRecent {
    pub sniped: Option<Username>,
    pub sniped_id: Option<u32>,
    pub sniper: Username,
    pub sniper_id: u32,
    pub mods: GameMods,
    pub map_id: u32,
    pub map: String,
    #[serde(with = "datetime_mixture")]
    pub date: OffsetDateTime,
    #[serde(with = "deser::adjust_acc")]
    pub accuracy: f32,
    #[serde(rename = "sr", deserialize_with = "deserialize_stars")]
    pub stars: Option<f32>,
}

// Format "YYYY-MM-DD hh:mm:ssZ"
mod datetime_mixture {
    use time::UtcOffset;

    use crate::util::datetime::OFFSET_FORMAT;

    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<OffsetDateTime, D::Error> {
        d.deserialize_str(DateTimeVisitor)
    }

    pub(super) struct DateTimeVisitor;

    impl<'de> Visitor<'de> for DateTimeVisitor {
        type Value = OffsetDateTime;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

pub fn deserialize_stars<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f32>, D::Error> {
    d.deserialize_any(StarsVisitor)
}

struct StarsVisitor;

impl<'de> Visitor<'de> for StarsVisitor {
    type Value = Option<f32>;

    #[inline]
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a stringified f32 or -1")
    }

    #[inline]
    fn visit_i64<E: Error>(self, _: i64) -> Result<Self::Value, E> {
        Ok(None)
    }

    #[inline]
    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        v.parse()
            .map(Some)
            .map_err(|_| Error::invalid_value(Unexpected::Str(v), &self))
    }
}

#[derive(Debug)]
pub struct SnipeScore {
    pub accuracy: f32,
    pub map_id: u32,
    pub mapset_id: u32,
    pub count_100: u32,
    pub count_50: u32,
    pub count_miss: u32,
    pub mods: GameMods,
    pub pp: Option<f32>,
    pub score: u32,
    pub score_date: OffsetDateTime,
    pub stars: f32,
    pub user_id: u32,
}

#[derive(Deserialize)]
struct InnerScore<'s> {
    player_id: u32,
    score: u32,
    pp: Option<f32>,
    mods: GameMods,
    accuracy: f32,
    count_100: u32,
    count_50: u32,
    count_miss: u32,
    date_set: &'s str,
    #[serde(default)]
    sr: Option<f32>,
}

impl<'de> Deserialize<'de> for SnipeScore {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct SnipeScoreVisitor;

        impl<'de> Visitor<'de> for SnipeScoreVisitor {
            type Value = SnipeScore;

            #[inline]
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("struct SnipeScore")
            }

            fn visit_map<V>(self, mut map: V) -> Result<SnipeScore, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut inner_score: Option<InnerScore<'_>> = None;
                let mut map_id = None;
                let mut mapset_id = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "map_id" => map_id = Some(map.next_value()?),
                        "set_id" => mapset_id = Some(map.next_value()?),
                        other if inner_score.is_none() && other.starts_with("top_") => {
                            inner_score = Some(map.next_value()?)
                        }
                        _ => {
                            let _ = map.next_value::<IgnoredAny>();
                        }
                    }
                }

                let inner_score = inner_score.ok_or_else(|| Error::missing_field("inner_score"))?;
                let map_id = map_id.ok_or_else(|| Error::missing_field("map_id"))?;
                let mapset_id = mapset_id.ok_or_else(|| Error::missing_field("mapset_id"))?;

                let date = PrimitiveDateTime::parse(inner_score.date_set, NAIVE_DATETIME_FORMAT)
                    .map(PrimitiveDateTime::assume_utc)
                    .unwrap_or_else(|err| {
                        warn!("Couldn't parse date `{}`: {err}", inner_score.date_set);

                        OffsetDateTime::now_utc()
                    });

                let score = SnipeScore {
                    accuracy: inner_score.accuracy * 100.0,
                    map_id,
                    mapset_id,
                    count_100: inner_score.count_100,
                    count_50: inner_score.count_50,
                    count_miss: inner_score.count_miss,
                    mods: inner_score.mods,
                    pp: inner_score.pp,
                    score: inner_score.score,
                    score_date: date,
                    stars: inner_score.sr.unwrap_or(0.0),
                    user_id: inner_score.player_id,
                };

                Ok(score)
            }
        }

        const FIELDS: &[&str] = &[
            "beatmap_id",
            "beatmapset_id",
            "user_id",
            "score",
            "pp",
            "mods",
            "accuracy",
            "count_100",
            "count_50",
            "count_miss",
            "score_date",
            "stars",
        ];

        d.deserialize_struct("SnipeScore", FIELDS, SnipeScoreVisitor)
    }
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
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

// Differs from `deser::option_datetime` in that failed string deserialization still returns `Ok(None)`
mod option_datetime {
    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<OffsetDateTime>, D::Error> {
        d.deserialize_option(OptionDateTimeVisitor)
    }

    struct OptionDateTimeVisitor;

    impl<'de> Visitor<'de> for OptionDateTimeVisitor {
        type Value = Option<OffsetDateTime>;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a string, preferably in `OffsetDateTime` format")
        }

        #[inline]
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            PrimitiveDateTime::parse(v, NAIVE_DATETIME_FORMAT)
                .ok()
                .map(PrimitiveDateTime::assume_utc)
                .map(Ok)
                .transpose()
        }

        #[inline]
        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            d.deserialize_str(self)
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
}
