use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct RespektiveUser {
    pub rank: u32,
    pub user_id: u32,
    #[serde(rename = "score")]
    pub ranked_score: u64,
}
