use super::str_to_u64;
use serde::{Deserialize, Deserializer};
use serde_derive::Deserialize as DeserializeDerive;

#[derive(DeserializeDerive, Debug)]
pub struct TwitchUser {
    #[serde(rename = "id", deserialize_with = "str_to_u64")]
    pub user_id: u64,
    #[serde(rename = "login")]
    pub login_name: String,
    pub display_name: String,
}

#[derive(DeserializeDerive)]
pub struct TwitchUsers {
    pub data: Vec<TwitchUser>,
}

#[derive(DeserializeDerive, Debug)]
pub struct TwitchStream {
    #[serde(rename = "game_id", deserialize_with = "str_to_u64")]
    pub game_id: u64,
    #[serde(rename = "id", deserialize_with = "str_to_u64")]
    pub stream_id: u64,
    pub thumbnail_url: String,
    pub title: String,
    #[serde(deserialize_with = "str_to_u64")]
    pub user_id: u64,
    #[serde(rename = "user_name")]
    pub username: String,
    #[serde(rename = "type", deserialize_with = "get_live")]
    pub live: bool,
}

impl TwitchStream {
    pub fn is_live(&self) -> bool {
        self.live
    }
}

fn get_live<'de, D>(d: D) -> std::result::Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(d)?;
    Ok(s == "live")
}

#[derive(DeserializeDerive)]
pub struct TwitchStreams {
    pub data: Vec<TwitchStream>,
}
