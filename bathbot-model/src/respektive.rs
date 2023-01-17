use super::deser;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Deserialize, Serialize)]
pub struct RespektiveUser {
    pub rank: u32,
    pub user_id: u32,
    #[serde(rename = "score")]
    pub ranked_score: u64,
}

#[derive(Deserialize)]
pub struct RespektiveTopCount {
    // pub beatmaps_amount: usize,
    pub user_id: u32,
    // pub username: Option<Username>,
    // pub country: Option<CountryCode>,
    pub top1s: usize,
    pub top1s_rank: Option<u32>,
    pub top8s: usize,
    pub top8s_rank: Option<u32>,
    pub top15s: usize,
    pub top15s_rank: Option<u32>,
    pub top25s: usize,
    pub top25s_rank: Option<u32>,
    pub top50s: usize,
    pub top50s_rank: Option<u32>,
    pub top100s: usize,
    pub top100s_rank: Option<u32>,
    #[serde(with = "deser::datetime")]
    pub last_update: OffsetDateTime,
}
