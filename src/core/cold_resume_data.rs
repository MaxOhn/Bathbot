use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Serialize, Debug)]
pub struct ColdRebootData {
    pub resume_data: HashMap<u64, (String, u64)>,
    pub shard_count: u64,
    pub total_shards: u64,
    pub guild_chunks: usize,
    pub user_chunks: usize,
}
