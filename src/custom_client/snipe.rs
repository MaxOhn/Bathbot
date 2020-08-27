use chrono::{offset::TimeZone, Date, DateTime, Utc};
use rosu::models::GameMods;
use serde::{
    de::{Deserializer, Error, MapAccess, Visitor},
    Deserialize,
};
use serde_derive::Deserialize as DerivedDeserialize;
use std::{
    collections::{BTreeMap, HashMap},
    convert::TryFrom,
    fmt,
};

#[derive(Debug, DerivedDeserialize)]
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
    #[serde(rename = "total_top_national_difference")]
    pub difference: i32,
    #[serde(rename = "mods_count", deserialize_with = "deserialize_mod_count")]
    pub count_mods: Option<Vec<(GameMods, u32)>>,
    #[serde(
        rename = "history_total_top_national",
        deserialize_with = "deserialize_history"
    )]
    pub count_first_history: BTreeMap<Date<Utc>, u32>,
    #[serde(rename = "sr_spread")]
    pub count_sr_spread: BTreeMap<u8, u32>,
    #[serde(rename = "oldest_date")]
    pub oldest_first: Option<SnipePlayerOldest>,
}

#[derive(Debug, DerivedDeserialize)]
pub struct SnipeCountryPlayer {
    #[serde(rename = "name")]
    pub username: String,
    pub user_id: u32,
    #[serde(rename = "average_pp")]
    pub avg_pp: f32,
    #[serde(rename = "average_sr")]
    pub avg_sr: f32,
    #[serde(rename = "count")]
    pub count_first: u32,
}

#[derive(Debug, DerivedDeserialize)]
pub struct SnipeTopDifference {
    pub name: String,
    #[serde(rename = "most_recent_top_national")]
    pub most_recent_firsts: Option<u32>,
    #[serde(rename = "total_top_national_difference")]
    pub difference: i32,
}

#[derive(Debug, DerivedDeserialize)]
pub struct SnipePlayerOldest {
    #[serde(rename = "map_id", deserialize_with = "deserialize_signed")]
    pub beatmap_id: u32,
    pub map: String,
    #[serde(deserialize_with = "deserialize_date")]
    pub date: DateTime<Utc>,
}

#[derive(Debug)]
pub struct SnipeScore {
    pub beatmap_id: u32,
    pub artist: String,
    pub title: String,
    pub version: String,
    pub user_id: u32,
    pub username: String,
    pub score: u32,
    pub pp: Option<f32>,
    pub mods: GameMods,
    pub tie: bool,
    pub accuracy: f32,
    pub count_100: u32,
    pub count_50: u32,
    pub count_miss: u32,
    pub date: DateTime<Utc>,
    pub stars: f32,
    pub max_combo: u32,
    pub bpm: f32,
    pub diff_ar: f32,
    pub diff_cs: f32,
    pub diff_hp: f32,
    pub diff_od: f32,
    pub seconds_total: u32,
}

#[derive(Debug)]
pub struct SnipeRecent {
    pub sniped: String,
    pub sniped_id: u32,
    pub sniper: String,
    pub sniper_id: u32,
    pub mods: GameMods,
    pub beatmap_id: u32,
    pub map: String,
    pub date: DateTime<Utc>,
    pub accuracy: f32,
    pub stars: f32,
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
                                    debug!("Couldn't deserialize `{}` into GameMods", other);
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
                        debug!("Couldn't deserialize `{}` into GameMods", other);
                        GameMods::NoMod
                    }),
                };
                let date = Utc
                    .datetime_from_str(&inner_score.date_set, "%F %T")
                    .unwrap_or_else(|why| {
                        debug!("Couldn't parse date `{}`: {}", inner_score.date_set, why);
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

pub fn deserialize_acc<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    Deserialize::deserialize(d).map(|n: f32| 100.0 * n)
}

pub fn deserialize_signed<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let n: i64 = Deserialize::deserialize(d)?;
    match n {
        n if n >= 0 && n <= u32::MAX as i64 => Ok(n as u32),
        _ => Ok(0),
    }
}

pub fn deserialize_date<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
    let date: &str = Deserialize::deserialize(d)?;
    Utc.datetime_from_str(&date, "%F %T").map_err(Error::custom)
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
        while let Some(key) = map.next_key()? {
            let mods = match key {
                "nomod" => GameMods::NoMod,
                _ => GameMods::try_from(key).unwrap_or_else(|_| {
                    debug!("Couldn't deserialize `{}` into GameMods", key);
                    GameMods::NoMod
                }),
            };
            mod_count.push((mods, map.next_value()?));
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
            let date = match chrono::NaiveDate::parse_from_str(key, "%F") {
                Ok(datetime) => Date::from_utc(datetime, Utc),
                Err(_) => return Err(Error::custom("could not parse date for history")),
            };
            history.insert(date, map.next_value()?);
        }
        Ok(history)
    }
}
