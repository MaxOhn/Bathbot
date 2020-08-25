use chrono::{offset::TimeZone, DateTime, Utc};
use rosu::models::GameMods;
use serde::{
    de::{Deserializer, Error, MapAccess, Visitor},
    Deserialize,
};
use serde_derive::Deserialize as DerivedDeserialize;
use std::{collections::HashMap, convert::TryFrom, fmt};

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
                        _ => debug!("Found unexpected key `{}`", key),
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
                        warn!("Couldn't deserialize `{}` into GameMods", other);
                        GameMods::NoMod
                    }),
                };
                let date = Utc
                    .datetime_from_str(&inner_score.date_set, "%F %T")
                    .unwrap_or_else(|why| {
                        warn!("Couldn't parse date `{}`: {}", inner_score.date_set, why);
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
