use std::collections::HashMap;

use rkyv::{Archive, Deserialize, Serialize};
use twilight_gateway::shard::ResumeSession;

pub use self::error::*;

mod defrost;
mod error;
mod freeze;

const STORE_DURATION: usize = 240; // seconds

const DATA_KEY: &str = "data";
const GUILD_KEY_PREFIX: &str = "guild_chunk";
const USER_KEY_PREFIX: &str = "user_chunk";
const MEMBER_KEY_PREFIX: &str = "member_chunk";
const CHANNEL_KEY_PREFIX: &str = "channel_chunk";
const ROLE_KEY_PREFIX: &str = "role_chunk";
const CURRENT_USER_KEY: &str = "current_user";

pub type ResumeData = HashMap<u64, ResumeSession>;

#[derive(Archive, Deserialize, Serialize)]
struct ColdResumeData {
    resume_data: ResumeData,
    guild_chunks: usize,
    user_chunks: usize,
    member_chunks: usize,
    channel_chunks: usize,
    role_chunks: usize,
}
