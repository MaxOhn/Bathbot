use super::deserialize::adjust_mode;

use chrono::{Date, DateTime, NaiveDate, Utc};
use rosu::models::GameMode;
use serde::{de, Deserialize, Deserializer};
use serde_json::Value;
use std::{collections::HashMap, fmt, ops::Deref};

#[derive(Debug)]
pub struct OsuAchievements(HashMap<u32, OsuAchievement>);

impl From<Vec<OsuAchievement>> for OsuAchievements {
    fn from(achievements: Vec<OsuAchievement>) -> Self {
        let achievements = achievements
            .into_iter()
            .map(|achievement| (achievement.achievement_id, achievement))
            .collect();
        Self(achievements)
    }
}

impl Deref for OsuAchievements {
    type Target = HashMap<u32, OsuAchievement>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for OsuAchievements {
    fn default() -> Self {
        Self(HashMap::default())
    }
}

#[derive(Debug, Deserialize)]
pub struct OsuAchievement {
    name: String,
    achievement_id: u32,
    description: String,
    grouping: String,
    icon_url: String,
    #[serde(rename = "id")]
    #[serde(deserialize_with = "trim_instructions")]
    instructions: Option<String>,
    #[serde(deserialize_with = "adjust_mode_maybe")]
    mode: Option<GameMode>,
    ordering: u32,
}

#[derive(Debug, Deserialize)]
pub struct DateCount {
    #[serde(deserialize_with = "str_to_date")]
    start_date: Date<Utc>,
    count: u32,
}

#[derive(Debug, Deserialize)]
pub struct OsuProfileKudosu {
    total: u32,
    available: u32,
}

#[derive(Debug)]
pub enum OsuProfilePlaystyle {
    Mouse,
    Keyboard,
    Tablet,
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
            _ => return Err(de::Error::custom(&format!("Unknown playstyle: {}", s))),
        };
        Ok(playstyle)
    }
}

#[derive(Debug, Deserialize)]
pub struct OsuProfileBadge {
    #[serde(deserialize_with = "str_to_datetime")]
    awarded_at: DateTime<Utc>,
    description: String,
    image_url: String,
}

#[derive(Debug, Deserialize)]
pub struct OsuProfileGrades {
    ss: u32,
    ssh: u32,
    s: u32,
    sh: u32,
    a: u32,
}

#[derive(Debug, Deserialize)]
pub struct OsuProfileStatistics {
    pp: f32,
    pp_rank: u32,
    ranked_score: u64,
    total_score: u64,
    #[serde(rename = "hit_accuracy")]
    accuracy: f32,
    #[serde(rename = "play_count")]
    playcount: u32,
    #[serde(rename = "play_time")]
    playtime: u32,
    total_hits: u32,
    #[serde(rename = "maximum_combo")]
    max_combo: u32,
    #[serde(rename = "replays_watched_by_others")]
    replays_watched: u32,
    grade_counts: OsuProfileGrades,
}

#[derive(Debug, Deserialize)]
pub struct OsuProfileAchievement {
    #[serde(deserialize_with = "str_to_datetime")]
    achieved_at: DateTime<Utc>,
    achievement_id: u32,
}

#[derive(Debug, Deserialize)]
pub struct OsuProfile {
    avatar_url: String,
    country_code: String,
    #[serde(rename = "id")]
    user_id: u32,
    is_active: bool,
    is_online: bool,
    is_supporter: bool,
    #[serde(deserialize_with = "str_to_maybe_datetime")]
    last_visit: Option<DateTime<Utc>>,
    username: String,
    cover_url: String,
    has_supported: bool,
    interests: Option<String>,
    #[serde(deserialize_with = "str_to_datetime")]
    join_date: DateTime<Utc>,
    kudosu: OsuProfileKudosu,
    location: Option<String>,
    occupation: Option<String>,
    #[serde(rename = "playmode", deserialize_with = "adjust_mode")]
    mode: GameMode,
    playstyle: Vec<OsuProfilePlaystyle>,
    post_count: u32,
    discord: Option<String>,
    twitter: Option<String>,
    website: Option<String>,
    is_admin: bool,
    is_bng: bool,
    is_full_bn: bool,
    is_gmt: bool,
    is_limited_bn: bool,
    is_moderator: bool,
    is_nat: bool,
    is_restricted: bool,
    is_silenced: bool,
    badges: Vec<OsuProfileBadge>,
    follower_count: u32,
    graveyard_beatmapset_count: u32,
    unranked_beatmapset_count: u32,
    loved_beatmapset_count: u32,
    ranked_and_approved_beatmapset_count: u32,
    monthly_playcounts: Vec<DateCount>,
    replays_watched_counts: Vec<DateCount>,
    scores_first_count: u32,
    statistics: OsuProfileStatistics,
    support_level: u32,
    #[serde(deserialize_with = "rank_history_vec")]
    rank_history: Option<Vec<u32>>,
    #[serde(rename = "user_achievements")]
    achievements: Vec<OsuProfileAchievement>,
}

pub fn adjust_mode_maybe<'de, D>(d: D) -> Result<Option<GameMode>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<&str> = Deserialize::deserialize(d)?;
    let m = s.map(|s| match s {
        "osu" => GameMode::STD,
        "taiko" => GameMode::TKO,
        "fruits" => GameMode::CTB,
        "mania" => GameMode::MNA,
        _ => panic!("Could not parse mode '{}'", s),
    });
    Ok(m)
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
        s.replace_range(0 + offset..=3 + offset, "");
        s
    }))
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

pub fn str_to_date<'de, D>(d: D) -> Result<Date<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(d)?;
    let naive_date = NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(de::Error::custom)?;
    Ok(Date::from_utc(naive_date, Utc))
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
