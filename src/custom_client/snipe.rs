use super::deserialize::str_to_date;

use chrono::{offset::TimeZone, Date, DateTime, Utc};
use rosu::models::GameMods;
use serde::{
    de::{Deserializer, Error, MapAccess, Visitor},
    Deserialize,
};
use serde_derive::Deserialize as DerivedDeserialize;
use std::{collections::HashMap, convert::TryFrom, fmt};

#[derive(Debug, DerivedDeserialize)]
pub struct SnipePlayerOldest {
    #[serde(rename = "map_id")]
    beatmap_id: u32,
    map: String,
    #[serde(deserialize_with = "str_to_date")]
    date: DateTime<Utc>,
}

#[derive(Debug)]
pub struct SnipePlayer {
    username: String,
    user_id: u32,
    avg_pp: f32,
    avg_acc: f32,
    avg_stars: f32,
    avg_score: f32,
    count_first: u32,
    count_loved: u32,
    count_ranked: u32,
    difference: i32,
    count_mods: HashMap<GameMods, u32>,
    count_first_history: HashMap<Date<Utc>, u32>,
    count_sr_spread: HashMap<u8, u32>,
    oldest_first: SnipePlayerOldest,
}

#[derive(Debug, DerivedDeserialize)]
pub struct SnipeCountryPlayer {
    #[serde(rename = "name")]
    username: String,
    user_id: u32,
    #[serde(rename = "average_pp")]
    avg_pp: f32,
    #[serde(rename = "average_sr")]
    avg_sr: f32,
    #[serde(rename = "count")]
    count_first: u32,
}

#[derive(Debug)]
pub enum SnipeTopType {
    Gain,
    Loss,
}

#[derive(Debug)]
pub struct SnipeTopDifference {
    top_type: SnipeTopType,
    name: String,
    most_recent_firsts: Option<u32>,
    difference: i32,
}

#[derive(Debug)]
pub struct SnipeScore {
    beatmap_id: u32,
    artist: String,
    title: String,
    version: String,
    user_id: u32,
    username: String,
    score: u32,
    pp: Option<f32>,
    mods: GameMods,
    tie: bool,
    accuracy: f32,
    count_100: u32,
    count_50: u32,
    count_miss: u32,
    date: DateTime<Utc>,
    stars: f32,
    max_combo: u32,
    bpm: f32,
    diff_ar: f32,
    diff_cs: f32,
    diff_hp: f32,
    diff_od: f32,
    seconds_total: u32,
}

#[derive(Debug)]
pub struct SnipeRecent {
    sniped: String,
    sniped_id: u32,
    sniper: String,
    sniper_id: u32,
    mods: GameMods,
    beatmap_id: u32,
    map: String,
    date: DateTime<Utc>,
    accuracy: f32,
    stars: f32,
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
                let mut sr_map: Option<HashMap<GameMods, f32>> = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        "_id" | "country" => {
                            let _: String = map.next_value()?;
                        }
                        "__v" => {
                            let _: f32 = map.next_value()?;
                        }
                        "date" => date = Some(map.next_value()?),
                        "map_id" => beatmap_id = Some(map.next_value()?),
                        "sniped" => sniped = Some(map.next_value()?),
                        "sniped_id" => sniped_id = Some(map.next_value()?),
                        "sniper" => sniper = Some(map.next_value()?),
                        "sniper_id" => sniper_id = Some(map.next_value()?),
                        "mods" => {
                            let mods_str: String = map.next_value()?;
                            mods = match mods_str.as_str() {
                                "nomod" => Some(GameMods::NoMod),
                                other => Some(GameMods::try_from(other).unwrap_or_else(|_| {
                                    println!("Couldn't deserialize `{}` into GameMods", other);
                                    GameMods::NoMod
                                })),
                            };
                        }
                        "accuracy" => accuracy = Some(map.next_value()?),
                        "map" => beatmap = Some(map.next_value()?),
                        "sr" => sr_map = Some(map.next_value()?),
                        _ => info!("Found unexpected key `{}`", key),
                    }
                }
                let mods = mods.ok_or_else(|| Error::missing_field("mods"))?;
                let stars = sr_map.ok_or_else(|| Error::missing_field("sr"))?;
                let (_, stars) =
                    stars
                        .into_iter()
                        .fold((0, 0.0), |(max_len, mod_sr), (curr_mods, sr)| {
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
                let date = Utc.datetime_from_str(&date, "%F %T").unwrap_or_else(|why| {
                    panic!("Could not parse date `{}`: {}", date, why);
                });
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

struct SnipePlayerModWrapper(HashMap<GameMods, u32>);

impl<'de> Deserialize<'de> for SnipePlayerModWrapper {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SnipePlayerModVisitor;

        impl<'de> Visitor<'de> for SnipePlayerModVisitor {
            type Value = SnipePlayerModWrapper;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut mod_count = HashMap::new();
                while let Some(key) = map.next_key()? {
                    let mods = match key {
                        "nomod" => GameMods::NoMod,
                        _ => GameMods::try_from(key).unwrap_or_else(|_| {
                            println!("Couldn't deserialize `{}` into GameMods", key);
                            GameMods::NoMod
                        }),
                    };
                    mod_count.insert(mods, map.next_value()?);
                }
                Ok(SnipePlayerModWrapper(mod_count))
            }
        }
        d.deserialize_map(SnipePlayerModVisitor)
    }
}

struct SnipePlayerHistoryWrapper(HashMap<Date<Utc>, u32>);

impl<'de> Deserialize<'de> for SnipePlayerHistoryWrapper {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SnipePlayerHistoryVisitor;

        impl<'de> Visitor<'de> for SnipePlayerHistoryVisitor {
            type Value = SnipePlayerHistoryWrapper;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut history = HashMap::new();
                while let Some(key) = map.next_key()? {
                    let date = match chrono::NaiveDate::parse_from_str(key, "%F") {
                        Ok(datetime) => Date::from_utc(datetime, Utc),
                        Err(why) => panic!("{} ({})", why, key),
                    };
                    history.insert(date, map.next_value()?);
                }
                Ok(SnipePlayerHistoryWrapper(history))
            }
        }
        d.deserialize_map(SnipePlayerHistoryVisitor)
    }
}

impl<'de> Deserialize<'de> for SnipePlayer {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SnipePlayerVisitor;

        impl<'de> Visitor<'de> for SnipePlayerVisitor {
            type Value = SnipePlayer;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct SnipePlayer")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut name = None;
                let mut user_id = None;
                let mut average_pp = None;
                let mut average_sr = None;
                let mut average_score = None;
                let mut average_accuracy = None;
                let mut oldest_date = None;
                let mut count_first_history: Option<SnipePlayerHistoryWrapper> = None;
                let mut difference = None;
                let mut sr_spread = None;
                let mut count = None;
                let mut mods_count: Option<SnipePlayerModWrapper> = None;
                let mut count_loved = None;
                let mut count_ranked = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        "name" => name = Some(map.next_value()?),
                        "user_id" => user_id = Some(map.next_value()?),
                        "average_pp" => average_pp = Some(map.next_value()?),
                        "average_sr" => average_sr = Some(map.next_value()?),
                        "average_score" => average_score = Some(map.next_value()?),
                        "average_accuracy" => average_accuracy = Some(map.next_value()?),
                        "oldest_date" => oldest_date = Some(map.next_value()?),
                        "history_total_top_national" => {
                            count_first_history = Some(map.next_value()?)
                        }
                        "total_top_national_difference" => difference = Some(map.next_value()?),
                        "sr_spread" => sr_spread = Some(map.next_value()?),
                        "count" => count = Some(map.next_value()?),
                        "mods_count" => mods_count = Some(map.next_value()?), // TODO
                        "count_loved" => count_loved = Some(map.next_value()?),
                        "count_ranked" => count_ranked = Some(map.next_value()?),
                        "_id" => {
                            let _: String = map.next_value()?;
                        }
                        rank if rank.starts_with("rank_") => {
                            let _: i32 = map.next_value()?;
                        }
                        "most_recent_top_national" | "rank_average_accuracy" | "count_sr" => {
                            let _: i32 = map.next_value()?;
                        }
                        "weighted_pp" | "bonus_pp" | "__v" => {
                            let _: f32 = map.next_value()?;
                        }
                        "total_score" => {
                            let _: i64 = map.next_value()?;
                        }
                        _ => info!("Found unexpected key `{}`", key),
                    }
                }
                let username = name.ok_or_else(|| Error::missing_field("name"))?;
                let user_id = user_id.ok_or_else(|| Error::missing_field("user_id"))?;
                let avg_pp = average_pp.ok_or_else(|| Error::missing_field("average_pp"))?;
                let avg_stars = average_sr.ok_or_else(|| Error::missing_field("average_sr"))?;
                let avg_score =
                    average_score.ok_or_else(|| Error::missing_field("average_score"))?;
                let avg_acc =
                    average_accuracy.ok_or_else(|| Error::missing_field("average_accuracy"))?;
                let oldest_first =
                    oldest_date.ok_or_else(|| Error::missing_field("oldest_date"))?;
                let count_first_history = count_first_history
                    .ok_or_else(|| Error::missing_field("history_total_top_national"))?;
                let difference = difference
                    .ok_or_else(|| Error::missing_field("total_top_national_difference"))?;
                let count_sr_spread = sr_spread.ok_or_else(|| Error::missing_field("sr_spread"))?;
                let count_first = count.ok_or_else(|| Error::missing_field("count"))?;
                let count_loved = count_loved.ok_or_else(|| Error::missing_field("count_loved"))?;
                let count_ranked =
                    count_ranked.ok_or_else(|| Error::missing_field("count_ranked"))?;
                let count_mods = mods_count.ok_or_else(|| Error::missing_field("mods_count"))?;

                let player = SnipePlayer {
                    username,
                    user_id,
                    avg_pp,
                    avg_acc,
                    avg_stars,
                    avg_score,
                    count_first,
                    count_loved,
                    count_ranked,
                    count_mods: count_mods.0,
                    count_first_history: count_first_history.0,
                    count_sr_spread,
                    oldest_first,
                    difference,
                };
                Ok(player)
            }
        }

        const FIELDS: &[&str] = &[
            "username",
            "user_id",
            "avg_pp",
            "avg_acc",
            "avg_stars",
            "avg_score",
            "count_first",
            "count_loved",
            "count_ranked",
            "difference",
            "count_mods",
            "count_first_history",
            "count_sr_spread",
            "oldest_first",
        ];
        d.deserialize_struct("SnipePlayer", FIELDS, SnipePlayerVisitor)
    }
}

impl<'de> Deserialize<'de> for SnipeTopDifference {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(DerivedDeserialize)]
        struct Inner {
            name: String,
            most_recent_top_national: Option<u32>,
            total_top_national_difference: i32,
        }
        let inner = Inner::deserialize(d)?;
        let top_type = if inner.most_recent_top_national.is_some() {
            SnipeTopType::Gain
        } else {
            SnipeTopType::Loss
        };
        Ok(Self {
            top_type,
            name: inner.name,
            most_recent_firsts: inner.most_recent_top_national,
            difference: inner.total_top_national_difference,
        })
    }
}

#[derive(DerivedDeserialize)]
struct InnerScore {
    player_id: u32,
    player: String,
    score: u32,
    pp: Option<f32>,
    mods: String,
    tie: bool,
    accuracy: f32,
    count_100: u32,
    count_50: u32,
    count_miss: u32,
    date_set: String,
    sr: f32,
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
                let mut artist = None;
                let mut title = None;
                let mut diff_name = None;
                let mut max_combo = None;
                let mut bpm = None;
                let mut ar = None;
                let mut cs = None;
                let mut hp = None;
                let mut od = None;
                let mut length = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        "map_id" => map_id = Some(map.next_value()?),
                        "artist" => artist = Some(map.next_value()?),
                        "title" => title = Some(map.next_value()?),
                        "diff_name" => diff_name = Some(map.next_value()?),
                        "max_combo" => max_combo = Some(map.next_value()?),
                        "bpm" => bpm = Some(map.next_value()?),
                        "ar" => ar = Some(map.next_value()?),
                        "cs" => cs = Some(map.next_value()?),
                        "hp" => hp = Some(map.next_value()?),
                        "od" => od = Some(map.next_value()?),
                        "length" => length = Some(map.next_value()?),
                        other if other.starts_with("top_") => inner_score = Some(map.next_value()?),
                        "new_star_ratings" => {
                            let _: HashMap<String, f32> = map.next_value()?;
                        }
                        "_id" => {
                            let _: String = map.next_value()?;
                        }
                        _ => info!("Found unexpected key `{}`", key),
                    }
                }
                let inner_score = inner_score.ok_or_else(|| Error::missing_field("inner_score"))?;
                let map_id = map_id.ok_or_else(|| Error::missing_field("map_id"))?;
                let artist = artist.ok_or_else(|| Error::missing_field("artist"))?;
                let title = title.ok_or_else(|| Error::missing_field("title"))?;
                let diff_name = diff_name.ok_or_else(|| Error::missing_field("diff_name"))?;
                let max_combo = max_combo.ok_or_else(|| Error::missing_field("max_combo"))?;
                let bpm = bpm.ok_or_else(|| Error::missing_field("bpm"))?;
                let ar = ar.ok_or_else(|| Error::missing_field("ar"))?;
                let cs = cs.ok_or_else(|| Error::missing_field("cs"))?;
                let hp = hp.ok_or_else(|| Error::missing_field("hp"))?;
                let od = od.ok_or_else(|| Error::missing_field("od"))?;
                let length = length.ok_or_else(|| Error::missing_field("length"))?;

                let mods = match inner_score.mods.as_str() {
                    "nomod" => GameMods::NoMod,
                    other => GameMods::try_from(other).unwrap_or_else(|_| {
                        println!("Couldn't deserialize `{}` into GameMods", other);
                        GameMods::NoMod
                    }),
                };
                let date = Utc
                    .datetime_from_str(&inner_score.date_set, "%F %T")
                    .unwrap_or_else(|why| {
                        println!("Couldn't parse date `{}`: {}", inner_score.date_set, why);
                        Utc::now()
                    });

                let score = SnipeScore {
                    beatmap_id: map_id,
                    artist,
                    title,
                    version: diff_name,
                    user_id: inner_score.player_id,
                    username: inner_score.player,
                    score: inner_score.score,
                    pp: inner_score.pp,
                    mods,
                    tie: inner_score.tie,
                    accuracy: inner_score.accuracy * 100.0,
                    count_100: inner_score.count_100,
                    count_50: inner_score.count_50,
                    count_miss: inner_score.count_miss,
                    date,
                    stars: inner_score.sr,
                    max_combo,
                    bpm,
                    diff_ar: ar,
                    diff_cs: cs,
                    diff_hp: hp,
                    diff_od: od,
                    seconds_total: length,
                };
                Ok(score)
            }
        }

        const FIELDS: &[&str] = &[
            "beatmap_id",
            "artist",
            "title",
            "version",
            "user_id",
            "username",
            "score",
            "pp",
            "mods",
            "tie",
            "accuracy",
            "count_100",
            "count_50",
            "count_miss",
            "date",
            "stars",
            "max_combo",
            "bpm",
            "diff_ar",
            "diff_cs",
            "diff_hp",
            "diff_od",
            "seconds_total",
        ];
        deserializer.deserialize_struct("SnipeScore", FIELDS, SnipeScoreVisitor)
    }
}
