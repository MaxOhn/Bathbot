use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub user_id: u16,
    pub username: String,
    pub join_date: String, // TODO
    pub count300: u32,
    pub count100: u32,
    pub count50: u32,
    pub playcount: u32,
    pub ranked_score: u64,
    pub total_score: u64,
    pub pp_rank: u32,
    pub level: f64,
    pub pp_raw: f64,
    pub accuracy: f64,
    pub count_rank_ssh: u32,
    pub count_rank_ss: u32,
    pub count_rank_sh: u32,
    pub count_rank_s: u32,
    pub count_rank_a: u32,
    pub country: String,
    pub total_seconds_played: u32,
    pub pp_country_rank: u32,
    //pub events: String TODO
}

impl User {
    pub fn new(
        user_id: u16,
        username: String,
        join_date: String,
        count300: u32,
        count100: u32,
        count50: u32,
        playcount: u32,
        ranked_score: u64,
        total_score: u64,
        pp_rank: u32,
        level: f64,
        pp_raw: f64,
        accuracy: f64,
        count_rank_ssh: u32,
        count_rank_ss: u32,
        count_rank_sh: u32,
        count_rank_s: u32,
        count_rank_a: u32,
        country: String,
        total_seconds_played: u32,
        pp_country_rank: u32,
    ) -> Self {
        Self {
            user_id,
            username,
            join_date,
            count300,
            count100,
            count50,
            playcount,
            ranked_score,
            total_score,
            pp_rank,
            level,
            pp_raw,
            accuracy,
            count_rank_ssh,
            count_rank_ss,
            count_rank_sh,
            count_rank_s,
            count_rank_a,
            country,
            total_seconds_played,
            pp_country_rank,
        }
    }

    pub fn default() -> Self {
        Self {
            user_id: 0,
            username: "".to_owned(),
            join_date: "".to_owned(),
            count300: 0,
            count100: 0,
            count50: 0,
            playcount: 0,
            ranked_score: 0,
            total_score: 0,
            pp_rank: 0,
            level: 0.0,
            pp_raw: 0.0,
            accuracy: 0.0,
            count_rank_ssh: 0,
            count_rank_ss: 0,
            count_rank_sh: 0,
            count_rank_s: 0,
            count_rank_a: 0,
            country: "".to_owned(),
            total_seconds_played: 0,
            pp_country_rank: 0,
        }
    }
}
