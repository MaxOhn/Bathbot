use super::deserialize::{expect_negative_u32, str_to_maybe_mode};

use chrono::{Date, DateTime, NaiveDate, Utc};
use rosu::model::GameMode;
use serde::{de, Deserialize, Deserializer};
use serde_json::Value;
use std::{collections::HashMap, fmt, ops::Deref};

#[derive(Debug, Deserialize)]
pub struct OsuProfile {
    pub country_code: String,
    #[serde(rename = "id")]
    pub user_id: u32,
    pub is_active: bool,
    pub is_online: bool,
    pub is_supporter: bool,
    #[serde(deserialize_with = "str_to_maybe_datetime")]
    pub last_visit: Option<DateTime<Utc>>,
    pub username: String,
    pub cover_url: String,
    pub has_supported: bool,
    #[serde(deserialize_with = "str_to_datetime")]
    pub join_date: DateTime<Utc>,
    pub kudosu: OsuProfileKudosu,
    pub interests: Option<String>,
    pub location: Option<String>,
    pub occupation: Option<String>,
    #[serde(rename = "playmode")]
    pub mode: GameMode,
    pub playstyle: Option<Vec<OsuProfilePlaystyle>>,
    pub post_count: u32,
    pub discord: Option<String>,
    pub twitter: Option<String>,
    pub website: Option<String>,
    pub is_admin: bool,
    pub is_bng: bool,
    pub is_full_bn: bool,
    pub is_gmt: bool,
    pub is_limited_bn: bool,
    pub is_moderator: bool,
    pub is_nat: bool,
    pub is_restricted: bool,
    pub is_silenced: bool,
    pub badges: Vec<OsuProfileBadge>,
    pub follower_count: u32,
    pub graveyard_beatmapset_count: u32,
    pub unranked_beatmapset_count: u32,
    pub loved_beatmapset_count: u32,
    pub ranked_and_approved_beatmapset_count: u32,
    pub monthly_playcounts: Vec<DateCount>,
    pub replays_watched_counts: Vec<DateCount>,
    pub scores_first_count: u32,
    pub statistics: OsuProfileStatistics,
    pub support_level: u32,
    #[serde(deserialize_with = "rank_history_vec")]
    pub rank_history: Option<Vec<u32>>,
    #[serde(rename = "user_achievements")]
    pub medals: Vec<OsuProfileMedal>,
}

#[derive(Debug)]
pub struct OsuMedals(HashMap<u32, OsuMedal>);

impl From<Vec<OsuMedal>> for OsuMedals {
    fn from(medals: Vec<OsuMedal>) -> Self {
        let medals = medals
            .into_iter()
            .map(|medal| (medal.medal_id, medal))
            .collect();
        Self(medals)
    }
}

impl Deref for OsuMedals {
    type Target = HashMap<u32, OsuMedal>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for OsuMedals {
    fn default() -> Self {
        Self(HashMap::default())
    }
}

#[derive(Debug, Deserialize)]
pub struct OsuMedal {
    pub name: String,
    #[serde(rename = "id")]
    pub medal_id: u32,
    pub description: String,
    pub grouping: String,
    pub icon_url: String,
    #[serde(deserialize_with = "trim_instructions")]
    pub instructions: Option<String>,
    #[serde(deserialize_with = "str_to_maybe_mode")]
    pub mode: Option<GameMode>,
    pub ordering: u32,
}

#[derive(Copy, Clone, Debug, Deserialize)]
pub struct DateCount {
    #[serde(deserialize_with = "str_to_date")]
    pub start_date: Date<Utc>,
    pub count: u32,
}

impl From<(Date<Utc>, u32)> for DateCount {
    fn from((start_date, count): (Date<Utc>, u32)) -> Self {
        Self { start_date, count }
    }
}

#[derive(Debug, Deserialize)]
pub struct OsuProfileKudosu {
    pub total: u32,
    pub available: i32,
}

#[derive(Debug)]
pub enum OsuProfilePlaystyle {
    Mouse,
    Keyboard,
    Tablet,
    Touch,
}

impl fmt::Display for OsuProfilePlaystyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl<'de> Deserialize<'de> for OsuProfilePlaystyle {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s: &str = Deserialize::deserialize(d)?;
        let playstyle = match s {
            "mouse" => Self::Mouse,
            "keyboard" => Self::Keyboard,
            "tablet" => Self::Tablet,
            "touch" => Self::Touch,
            _ => return Err(de::Error::custom(&format!("Unknown playstyle `{}`", s))),
        };
        Ok(playstyle)
    }
}

#[derive(Debug, Deserialize)]
pub struct OsuProfileBadge {
    #[serde(deserialize_with = "str_to_datetime")]
    pub awarded_at: DateTime<Utc>,
    pub description: String,
    pub image_url: String,
}

#[derive(Debug, Deserialize)]
pub struct OsuProfileGrades {
    #[serde(deserialize_with = "expect_negative_u32")]
    pub ss: u32,
    #[serde(deserialize_with = "expect_negative_u32")]
    pub ssh: u32,
    #[serde(deserialize_with = "expect_negative_u32")]
    pub s: u32,
    #[serde(deserialize_with = "expect_negative_u32")]
    pub sh: u32,
    #[serde(deserialize_with = "expect_negative_u32")]
    pub a: u32,
}

#[derive(Debug, Deserialize)]
pub struct OsuProfileStatistics {
    pub pp: f32,
    pub pp_rank: u32,
    pub ranked_score: u64,
    pub total_score: u64,
    #[serde(rename = "hit_accuracy")]
    pub accuracy: f32,
    #[serde(rename = "play_count")]
    pub playcount: u32,
    #[serde(rename = "play_time", deserialize_with = "defaulting_u32")]
    pub playtime: u32,
    pub total_hits: u32,
    #[serde(rename = "maximum_combo")]
    pub max_combo: u32,
    #[serde(rename = "replays_watched_by_others")]
    pub replays_watched: u32,
    pub grade_counts: OsuProfileGrades,
}

#[derive(Debug, Deserialize)]
pub struct OsuProfileMedal {
    #[serde(deserialize_with = "str_to_datetime")]
    pub achieved_at: DateTime<Utc>,
    pub medal_id: u32,
}

/// Trimming <i> and </i>
pub fn trim_instructions<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Deserialize::deserialize(d)?;
    Ok(s.map(|mut s| {
        s.replace_range(0..=2, "");
        let offset = s.chars().count() - 4;
        s.replace_range(offset..=3 + offset, "");
        s
    }))
}

pub fn str_to_date<'de, D>(d: D) -> Result<Date<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(d)?;
    let naive_date = NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(de::Error::custom)?;
    Ok(Date::from_utc(naive_date, Utc))
}

pub fn defaulting_u32<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let u: Option<u32> = Deserialize::deserialize(d)?;
    Ok(u.unwrap_or_default())
}

pub fn rank_history_vec<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<u32>>, D::Error> {
    let value: Option<Value> = Deserialize::deserialize(d)?;
    let mut value = match value {
        Some(value) => value,
        None => return Ok(None),
    };
    let data: Vec<_> = value
        .get_mut("data")
        .unwrap()
        .take()
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|n| n.as_u64())
        .map(|n| n as u32)
        .collect();
    Ok(Some(data))
}

pub fn str_to_maybe_datetime<'de, D>(d: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Deserialize::deserialize(d)?;
    s.map(|s| DateTime::parse_from_rfc3339(&s).map(|date| date.with_timezone(&Utc)))
        .transpose()
        .map_err(de::Error::custom)
}

pub fn str_to_datetime<'de, D>(d: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(d)?;
    DateTime::parse_from_rfc3339(&s)
        .map(|date| date.with_timezone(&Utc))
        .map_err(de::Error::custom)
}
