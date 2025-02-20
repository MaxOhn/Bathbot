#![allow(dead_code)]
use crate::deser::{datetime_rfc3339, option_datetime_rfc3339};
use serde::Deserialize;
use time::OffsetDateTime;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxScore {
    id: u64,
    user_id: u32,
    user: RelaxUser,
    beatmap_id: u32,
    beatmap: RelaxBeatmap,
    grade: RelaxGrade,
    accuracy: f64,
    combo: u32,
    mods: Option<String>,
    #[serde(with = "datetime_rfc3339")]
    date: OffsetDateTime,
    total_score: u32,
    count_50: u32,
    count_100: u32,
    count_300: u32,
    count_miss: u32,
    spinner_bonus: Option<u32>,
    spinner_spins: Option<u32>,
    legacy_slider_ends: Option<u32>,
    slider_ticks: Option<u32>,
    slider_ends: Option<u32>,
    legacy_slider_end_misses: Option<u32>,
    slider_tick_misses: Option<u32>,
    pp: Option<f64>,
    is_best: bool,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxUser {
    pub id: u32,
    pub country_code: Option<String>,
    pub username: Option<String>,
    pub total_pp: Option<f64>,
    pub total_accuracy: Option<f64>,
    #[serde(with = "option_datetime_rfc3339")]
    pub updated_at: Option<OffsetDateTime>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxBeatmap {
    id: u32,
    artist: Option<String>,
    title: Option<String>,
    creator_id: u32,
    beatmap_set_id: u32,
    difficulty_name: Option<String>,
    approach_rate: f64,
    overall_difficulty: f64,
    circle_size: f64,
    health_drain: f64,
    beats_per_minute: f64,
    circles: u32,
    sliders: u32,
    spinners: u32,
    star_rating_normal: f64,
    star_rating: Option<f64>,
    status: RelaxBeatmapStatus,
    max_combo: u32,
}

#[derive(Debug, Deserialize)]
pub enum RelaxBeatmapStatus {
    #[serde(rename = "Graveyard")]
    Graveyard,
    #[serde(rename = "Wip")]
    Wip,
    #[serde(rename = "Pending")]
    Pending,
    #[serde(rename = "Ranked")]
    Ranked,
    #[serde(rename = "Approved")]
    Approved,
    #[serde(rename = "Qualified")]
    Qualified,
    #[serde(rename = "Loved")]
    Loved,
}

#[derive(Debug, Deserialize)]
pub enum RelaxGrade {
    #[serde(rename = "F")]
    F,
    #[serde(rename = "D")]
    D,
    #[serde(rename = "C")]
    C,
    #[serde(rename = "B")]
    B,
    #[serde(rename = "A")]
    A,
    #[serde(rename = "S")]
    S,
    #[serde(rename = "SH")]
    SH,
    #[serde(rename = "X")]
    X,
    #[serde(rename = "XH")]
    XH,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxAllowedModsResponse {
    mods: Option<Vec<String>>,
    mod_settings: Option<Vec<String>>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxListingBeatmap {
    id: u32,
    artist: Option<String>,
    title: Option<String>,
    creator_id: u32,
    beatmap_set_id: u32,
    difficulty_name: Option<String>,
    star_rating: Option<f32>,
    status: RelaxBeatmapStatus,
    playcount: u32,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxPlaycountPerMonth {
    // TODO: Make date-time
    date: String,
    playcount: u32,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxRecentScoresResponse {
    scores: Option<Vec<RelaxScore>>,
    scores_today: u32,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxStatsResponse {
    scores_total: u32,
    users_total: u32,
    beatmaps_total: u32,
    latest_score_id: u64,
    scores_in_a_month: u32,
    playcount_per_day: Option<RelaxPlaycountPerMonth>,
    playcount_per_month: Option<RelaxPlaycountPerMonth>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxPlayersDataResponse {
    pub id: u32,
    pub country_code: Option<String>,
    pub username: Option<String>,
    pub total_pp: Option<f64>,
    pub total_accuracy: Option<f64>,
    // TODO: Make date-time
    pub updated_at: Option<String>,
    pub rank: Option<u32>,
    pub country_rank: Option<u32>,
    pub playcount: u32,
    #[serde(rename = "countSS")]
    pub count_ss: u32,
    #[serde(rename = "countS")]
    pub count_s: u32,
    #[serde(rename = "countA")]
    pub count_a: u32,
    pub playcounts_per_month: Vec<Option<RelaxPlaycountPerMonth>>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxPlayersResult {
    players: Vec<Option<RelaxUser>>,
    total: u32,
}
