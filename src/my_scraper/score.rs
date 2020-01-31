// Alternative response from the API, currently unused since requests cannot
// contain custom cookies for now
#![allow(dead_code)]

//use serde_derive::Deserialize;

//#[derive(Deserialize)]
pub struct ScraperScore {
    id: u64,
    user_id: u32,
    accuracy: f32,
    mods: Vec<String>,
    score: u32,
    max_combo: u32,
    perfect: bool,
    statistics: ScraperScoreStatistics,
    pp: f32,
    rank: String,
    created_at: String,
    mode_int: u8,
    replay: bool,
    beatmap: ScraperBeatmap,
    user: ScraperUser,
}

//#[derive(Deserialize)]
pub struct ScraperScoreStatistics {
    count50: u32,
    count100: u32,
    count300: u32,
    count_geki: u32,
    count_katu: u32,
    count_miss: u32,
}

//#[derive(Deserialize)]
pub struct ScraperBeatmap {
    id: u32,
    beatmapset_id: u32,
    mode_int: u32,
    difficulty_rating: f32,
    version: String,
    total_length: u32,
    hit_length: u32,
    bpm: u32,
    cs: f32,
    drain: f32,
    accuracy: f32,
    ar: f32,
    playcount: u32,
    passcount: u32,
    count_circles: u32,
    count_sliders: u32,
    count_spinner: u32,
    count_total: u32,
    last_updated: String,
    ranked: u8,
}

//#[derive(Deserialize)]
pub struct ScraperUser {
    id: u32,
    username: String,
    country_code: String,
}
