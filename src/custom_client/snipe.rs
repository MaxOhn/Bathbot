use super::deserialize::{expect_negative_u32, str_to_maybe_datetime};
use crate::util::osu::ModSelection;

use chrono::{offset::TimeZone, Date, DateTime, NaiveDate, Utc};
use rosu::model::{GameMode, GameMods};
use serde::{
    de::{Deserializer, Error, IgnoredAny, MapAccess, Unexpected, Visitor},
    Deserialize,
};
use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    str::FromStr,
};

#[derive(Copy, Clone, Debug)]
pub enum SnipeScoreOrder {
    Accuracy = 0,
    Length = 1,
    MapApprovalDate = 2,
    Misses = 3,
    Pp = 4,
    ScoreDate = 5,
    Stars = 6,
}

impl fmt::Display for SnipeScoreOrder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            Self::Accuracy => "accuracy",
            Self::Length => "length",
            Self::MapApprovalDate => "date_ranked",
            Self::Misses => "count_miss",
            Self::Pp => "pp",
            Self::ScoreDate => "date_set",
            Self::Stars => "sr",
        };
        f.write_str(name)
    }
}

#[derive(Debug)]
pub struct SnipeScoreParams {
    pub user_id: u32,
    pub country: String,
    pub page: u8,
    pub mode: GameMode,
    pub order: SnipeScoreOrder,
    pub mods: Option<ModSelection>,
    pub descending: bool,
}

impl SnipeScoreParams {
    pub fn new(user_id: u32, country_code: impl Into<String>) -> Self {
        Self {
            user_id,
            country: country_code.into().to_lowercase(),
            page: 0,
            mode: GameMode::STD,
            order: SnipeScoreOrder::Pp,
            mods: None,
            descending: true,
        }
    }
    #[allow(dead_code)]
    pub fn mode(mut self, mode: GameMode) -> Self {
        self.mode = mode;
        self
    }
    pub fn order(mut self, order: SnipeScoreOrder) -> Self {
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
pub struct SnipePlayer {
    #[serde(rename = "name")]
    pub username: String,
    pub user_id: u32,
    #[serde(rename = "average_pp")]
    pub avg_pp: f32,
    #[serde(rename = "average_accuracy", deserialize_with = "deserialize_acc")]
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
    #[serde(
        rename = "mods_count",
        deserialize_with = "deserialize_mod_count",
        default
    )]
    pub count_mods: Option<Vec<(GameMods, u32)>>,
    #[serde(
        rename = "history_total_top_national",
        deserialize_with = "deserialize_history",
        default
    )]
    pub count_first_history: BTreeMap<Date<Utc>, u32>,
    #[serde(rename = "sr_spread")]
    pub count_sr_spread: BTreeMap<u8, u32>,
    #[serde(rename = "oldest_date")]
    pub oldest_first: Option<SnipePlayerOldest>,
}

#[derive(Debug, Deserialize)]
pub struct SnipeCountryPlayer {
    #[serde(rename = "name")]
    pub username: String,
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
pub struct SnipeTopDifference {
    pub name: String,
    #[serde(rename = "most_recent_top_national")]
    pub most_recent_firsts: Option<u32>,
    #[serde(rename = "total_top_national_difference")]
    pub difference: i32,
}

#[derive(Debug, Deserialize)]
pub struct SnipePlayerOldest {
    #[serde(rename = "map_id", deserialize_with = "expect_negative_u32")]
    pub beatmap_id: u32,
    pub map: String,
    #[serde(deserialize_with = "str_to_maybe_datetime")]
    pub date: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct SnipeRecent {
    pub sniped: Option<String>,
    pub sniped_id: Option<u32>,
    pub sniper: String,
    pub sniper_id: u32,
    pub mods: GameMods,
    pub beatmap_id: u32,
    pub map: String,
    pub date: DateTime<Utc>,
    pub accuracy: f32,
    pub stars: Option<f32>,
}

impl<'de> Deserialize<'de> for SnipeRecent {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SnipeRecentVisitor;

        impl<'de> Visitor<'de> for SnipeRecentVisitor {
            type Value = SnipeRecent;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct SnipeRecent")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut sniped = None;
                let mut sniped_id = None;
                let mut sniper = None;
                let mut sniper_id = None;
                let mut mods: Option<GameMods> = None;
                let mut beatmap_id = None;
                let mut beatmap = None;
                let mut date: Option<String> = None;
                let mut accuracy = None;
                let mut sr_map: Option<HashMap<GameMods, Option<f32>>> = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        "date" => date = Some(map.next_value()?),
                        "map_id" => beatmap_id = Some(map.next_value()?),
                        "sniped" => sniped = Some(map.next_value()?),
                        "sniped_id" => sniped_id = Some(map.next_value()?),
                        "sniper" => sniper = Some(map.next_value()?),
                        "sniper_id" => sniper_id = Some(map.next_value()?),
                        "mods" => mods = Some(map.next_value()?),
                        "accuracy" => accuracy = Some(map.next_value()?),
                        "map" => beatmap = Some(map.next_value()?),
                        "sr" => sr_map = Some(map.next_value()?),
                        _ => {
                            let _ = map.next_value::<IgnoredAny>();
                        }
                    }
                }
                let mods = mods.ok_or_else(|| Error::missing_field("mods"))?;
                let stars = sr_map.ok_or_else(|| Error::missing_field("sr"))?;
                let (_, stars) =
                    stars
                        .into_iter()
                        .fold((0, None), |(max_len, mod_sr), (curr_mods, sr)| {
                            let len = (mods & curr_mods).len();
                            if max_len < len {
                                (len, sr)
                            } else {
                                (max_len, mod_sr)
                            }
                        });
                let sniped = sniped.ok_or_else(|| Error::missing_field("sniped"))?;
                let sniped_id = sniped_id.ok_or_else(|| Error::missing_field("sniped_id"))?;
                let sniper = sniper.ok_or_else(|| Error::missing_field("sniper"))?;
                let sniper_id = sniper_id.ok_or_else(|| Error::missing_field("sniper_id"))?;
                let beatmap_id = beatmap_id.ok_or_else(|| Error::missing_field("beatmap_id"))?;
                let date = date.ok_or_else(|| Error::missing_field("date"))?;
                let date = Utc.datetime_from_str(&date, "%F %T").map_err(|_| {
                    Error::invalid_value(Unexpected::Str(&date), &"a date of the form `%F %T`")
                })?;
                let accuracy = accuracy.ok_or_else(|| Error::missing_field("accuracy"))?;
                let beatmap = beatmap.ok_or_else(|| Error::missing_field("map"))?;

                let snipe = SnipeRecent {
                    sniped,
                    sniped_id,
                    sniper,
                    sniper_id,
                    mods,
                    beatmap_id,
                    map: beatmap,
                    date,
                    accuracy,
                    stars,
                };
                Ok(snipe)
            }
        }

        const FIELDS: &[&str] = &[
            "sniped",
            "sniped_id",
            "sniper",
            "sniper_id",
            "mods",
            "beatmap_id",
            "map",
            "date",
            "accuracy",
            "stars",
        ];
        d.deserialize_struct("SnipeRecent", FIELDS, SnipeRecentVisitor)
    }
}

#[derive(Debug)]
pub struct SnipeScore {
    pub accuracy: f32,
    // pub artist: String,
    pub beatmap_id: u32,
    pub beatmapset_id: u32,
    // pub bpm: f32,
    pub count_100: u32,
    pub count_50: u32,
    pub count_miss: u32,
    // pub diff_ar: f32,
    // pub diff_cs: f32,
    // pub diff_hp: f32,
    // pub diff_od: f32,
    // pub map_approved_date: DateTime<Utc>,
    // pub map_max_combo: u32,
    pub mods: GameMods,
    pub pp: Option<f32>,
    pub score: u32,
    pub score_date: DateTime<Utc>,
    // pub seconds_total: u32,
    pub stars: f32,
    // pub tie: bool,
    // pub title: String,
    pub user_id: u32,
    // pub username: String,
    // pub version: String,
}

#[derive(Deserialize)]
struct InnerScore<'s> {
    player_id: u32,
    // player: String,
    score: u32,
    pp: Option<f32>,
    mods: GameMods,
    // tie: bool,
    accuracy: f32,
    count_100: u32,
    count_50: u32,
    count_miss: u32,
    date_set: &'s str,
    sr: Option<f32>,
}

impl<'de> Deserialize<'de> for SnipeScore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SnipeScoreVisitor;

        impl<'de> Visitor<'de> for SnipeScoreVisitor {
            type Value = SnipeScore;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct SnipeScore")
            }

            fn visit_map<V>(self, mut map: V) -> Result<SnipeScore, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut inner_score: Option<InnerScore> = None;
                let mut map_id = None;
                let mut mapset_id = None;
                let mut star_ratings: Option<HashMap<&str, f32>> = None;
                // let mut artist = None;
                // let mut title = None;
                // let mut diff_name = None;
                // let mut max_combo = None;
                // let mut bpm = None;
                // let mut ar = None;
                // let mut cs = None;
                // let mut hp = None;
                // let mut od = None;
                // let mut date_ranked = None;
                // let mut length = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        "map_id" => map_id = Some(map.next_value()?),
                        "set_id" => mapset_id = Some(map.next_value()?),
                        "new_star_ratings" => star_ratings = Some(map.next_value()?),
                        // "artist" => artist = Some(map.next_value()?),
                        // "date_ranked" => {
                        //     let date: &str = map.next_value()?;
                        //     let date = Utc.datetime_from_str(date, "%F %T").unwrap_or_else(|why| {
                        //         warn!("Couldn't parse date `{}`: {}", date, why);
                        //         Utc::now()
                        //     });
                        //     date_ranked = Some(date);
                        // }
                        // "title" => title = Some(map.next_value()?),
                        // "diff_name" => diff_name = Some(map.next_value()?),
                        // "max_combo" => max_combo = Some(map.next_value()?),
                        // "bpm" => bpm = Some(map.next_value()?),
                        // "ar" => ar = Some(map.next_value()?),
                        // "cs" => cs = Some(map.next_value()?),
                        // "hp" => hp = Some(map.next_value()?),
                        // "od" => od = Some(map.next_value()?),
                        // "length" => length = Some(map.next_value()?),
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
                // let artist = artist.ok_or_else(|| Error::missing_field("artist"))?;
                // let title = title.ok_or_else(|| Error::missing_field("title"))?;
                // let diff_name = diff_name.ok_or_else(|| Error::missing_field("diff_name"))?;
                // let max_combo = max_combo.ok_or_else(|| Error::missing_field("max_combo"))?;
                // let bpm = bpm.ok_or_else(|| Error::missing_field("bpm"))?;
                // let ar = ar.ok_or_else(|| Error::missing_field("ar"))?;
                // let cs = cs.ok_or_else(|| Error::missing_field("cs"))?;
                // let hp = hp.ok_or_else(|| Error::missing_field("hp"))?;
                // let od = od.ok_or_else(|| Error::missing_field("od"))?;
                // let length = length.ok_or_else(|| Error::missing_field("length"))?;
                // let map_approved_date =
                //     date_ranked.ok_or_else(|| Error::missing_field("date_ranked"))?;

                let mods = inner_score.mods;

                let date = Utc
                    .datetime_from_str(inner_score.date_set, "%F %T")
                    .unwrap_or_else(|why| {
                        warn!("Couldn't parse date `{}`: {}", inner_score.date_set, why);
                        Utc::now()
                    });

                let stars = inner_score.sr.unwrap_or_else(|| {
                    star_ratings
                        .and_then(|srs| {
                            srs.into_iter()
                                .find(|(m, _)| GameMods::from_str(m).map_or(false, |m| m == mods))
                        })
                        .map_or(0.0, |(_, sr)| sr)
                });

                let score = SnipeScore {
                    accuracy: inner_score.accuracy * 100.0,
                    // artist,
                    beatmap_id: map_id,
                    beatmapset_id: mapset_id,
                    // bpm,
                    count_100: inner_score.count_100,
                    count_50: inner_score.count_50,
                    count_miss: inner_score.count_miss,
                    // diff_ar: ar,
                    // diff_cs: cs,
                    // diff_hp: hp,
                    // diff_od: od,
                    mods,
                    // map_approved_date,
                    // map_max_combo: max_combo,
                    pp: inner_score.pp,
                    score: inner_score.score,
                    score_date: date,
                    stars,
                    // seconds_total: length,
                    // tie: inner_score.tie,
                    // title,
                    user_id: inner_score.player_id,
                    // username: inner_score.player,
                    // version: diff_name,
                };
                Ok(score)
            }
        }

        const FIELDS: &[&str] = &[
            "beatmap_id",
            "beatmapset_id",
            // "artist",
            // "title",
            // "version",
            "user_id",
            // "username",
            "score",
            "pp",
            "mods",
            // "tie",
            "accuracy",
            "count_100",
            "count_50",
            "count_miss",
            "score_date",
            "stars",
            // "max_combo",
            // "bpm",
            // "diff_ar",
            // "diff_cs",
            // "diff_hp",
            // "diff_od",
            // "seconds_total",
        ];
        deserializer.deserialize_struct("SnipeScore", FIELDS, SnipeScoreVisitor)
    }
}

pub fn deserialize_acc<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    Deserialize::deserialize(d).map(|n: f32| 100.0 * n)
}

fn deserialize_mod_count<'de, D>(d: D) -> Result<Option<Vec<(GameMods, u32)>>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(d.deserialize_map(SnipePlayerModVisitor).ok())
}

struct SnipePlayerModVisitor;

impl<'de> Visitor<'de> for SnipePlayerModVisitor {
    type Value = Vec<(GameMods, u32)>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map")
    }

    fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
    where
        V: MapAccess<'de>,
    {
        let mut mod_count = Vec::new();
        while let Some((mods, num)) = map.next_entry()? {
            mod_count.push((mods, num));
        }
        Ok(mod_count)
    }
}

fn deserialize_history<'de, D>(d: D) -> Result<BTreeMap<Date<Utc>, u32>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(d.deserialize_map(SnipePlayerHistoryVisitor)?)
}

struct SnipePlayerHistoryVisitor;

impl<'de> Visitor<'de> for SnipePlayerHistoryVisitor {
    type Value = BTreeMap<Date<Utc>, u32>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map")
    }

    fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
    where
        V: MapAccess<'de>,
    {
        let mut history = BTreeMap::new();
        while let Some(key) = map.next_key()? {
            let naive_date = NaiveDate::parse_from_str(key, "%F").map_err(|_| {
                Error::invalid_value(Unexpected::Str(key), &"a date of  the form `%F`")
            })?;
            let date = Date::from_utc(naive_date, Utc);
            history.insert(date, map.next_value()?);
        }
        Ok(history)
    }
}
