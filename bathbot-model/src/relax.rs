#![allow(dead_code)]
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RelaxScore {
    #[serde(rename = "id")]
    id: u64,
    #[serde(rename = "userId")]
    user_id: u32,
    #[serde(rename = "user")]
    user: RelaxUser,
    #[serde(rename = "beatmapId")]
    beatmap_id: u32,
    #[serde(rename = "beatmap")]
    beatmap: RelaxBeatmap,
    #[serde(rename = "grade")]
    grade: RelaxGrade,
    #[serde(rename = "accuracy")]
    accuracy: f64,
    #[serde(rename = "combo")]
    combo: u32,
    #[serde(rename = "mods")]
    mods: Option<String>,

    // Make date-time
    #[serde(rename = "date")]
    date: String,
    #[serde(rename = "totalScore")]
    total_score: u32,
    #[serde(rename = "count50")]
    count_50: u32,
    #[serde(rename = "count100")]
    count_100: u32,
    #[serde(rename = "count300")]
    count_300: u32,
    #[serde(rename = "countMiss")]
    count_miss: u32,
    #[serde(rename = "spinnerBonus")]
    spinner_bonus: Option<u32>,
    #[serde(rename = "spinnerSpins")]
    spinner_spins: Option<u32>,
    #[serde(rename = "legacySliderEnds")]
    legacy_slider_ends: Option<u32>,
    #[serde(rename = "sliderTicks")]
    slider_ticks: Option<u32>,
    #[serde(rename = "sliderEnds")]
    slider_ends: Option<u32>,
    #[serde(rename = "legacySliderEndMisses")]
    legacy_slider_end_misses: Option<u32>,
    #[serde(rename = "sliderTickMisses")]
    slider_tick_misses: Option<u32>,
    #[serde(rename = "pp")]
    pp: Option<f64>,
    #[serde(rename = "isBest")]
    is_best: bool,
}
#[derive(Debug, Deserialize)]
pub struct RelaxUser {
    #[serde(rename = "id")]
    pub id: u32,
    #[serde(rename = "countryCode")]
    pub country_code: Option<String>,
    #[serde(rename = "username")]
    pub username: Option<String>,
    #[serde(rename = "totalPp")]
    pub total_pp: Option<f64>,
    #[serde(rename = "totalAccuracy")]
    pub total_accuracy: Option<f64>,
    // Make date-time
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct RelaxBeatmap {
    #[serde(rename = "id")]
    id: u32,
    #[serde(rename = "artist")]
    artist: Option<String>,
    #[serde(rename = "title")]
    title: Option<String>,
    #[serde(rename = "creatorId")]
    creator_id: u32,
    #[serde(rename = "beatmapSetId")]
    beatmap_set_id: u32,
    #[serde(rename = "difficultyName")]
    difficulty_name: Option<String>,
    #[serde(rename = "approachRate")]
    approach_rate: f64,
    #[serde(rename = "overallDifficulty")]
    overall_difficulty: f64,
    #[serde(rename = "circleSize")]
    circle_size: f64,
    #[serde(rename = "healthDrain")]
    health_drain: f64,
    #[serde(rename = "beatsPerMinute")]
    beats_per_minute: f64,
    #[serde(rename = "circles")]
    circles: u32,
    #[serde(rename = "sliders")]
    sliders: u32,
    #[serde(rename = "spinners")]
    spinners: u32,
    #[serde(rename = "starRatingNormal")]
    star_rating_normal: f64,
    #[serde(rename = "starRating")]
    star_rating: Option<f64>,
    #[serde(rename = "status")]
    status: RelaxBeatmapStatus,
    #[serde(rename = "maxCombo")]
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
