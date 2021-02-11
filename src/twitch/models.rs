use serde::{de, Deserialize, Deserializer};
use std::str::FromStr;

fn str_to_u64<'de, D>(d: D) -> std::result::Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(d)?;
    u64::from_str(s).map_err(de::Error::custom)
}

fn str_to_maybe_u64<'de, D>(d: D) -> std::result::Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(d)?;

    if s.is_empty() {
        Ok(None)
    } else {
        u64::from_str(s).map(Some).map_err(de::Error::custom)
    }
}

#[derive(Deserialize, Debug)]
pub struct TwitchUser {
    #[serde(rename = "id", deserialize_with = "str_to_u64")]
    pub user_id: u64,
    #[serde(rename = "login")]
    pub display_name: String,
    #[serde(rename = "profile_image_url")]
    pub image_url: String,
}

#[derive(Deserialize)]
pub struct TwitchUsers {
    pub data: Vec<TwitchUser>,
}

#[derive(Deserialize, Debug)]
pub struct TwitchStream {
    #[serde(rename = "game_id", deserialize_with = "str_to_maybe_u64")]
    pub game_id: Option<u64>,
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
    #[inline]
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

#[derive(Deserialize)]
pub struct TwitchStreams {
    pub data: Vec<TwitchStream>,
}
