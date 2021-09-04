use chrono::{DateTime, Utc};
use serde::{de::Error, Deserialize, Deserializer};

fn str_to_u64<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    <&str as Deserialize>::deserialize(d)?
        .parse()
        .map_err(Error::custom)
}

fn str_to_maybe_u64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    let s: &str = Deserialize::deserialize(d)?;

    if s.is_empty() {
        Ok(None)
    } else {
        s.parse().map(Some).map_err(Error::custom)
    }
}

// TODO: Serialize
#[derive(Debug, Deserialize)]
pub struct TwitchUser {
    #[serde(rename = "id", deserialize_with = "str_to_u64")]
    pub user_id: u64,
    pub description: String,
    #[serde(rename = "login")]
    pub display_name: String,
    #[serde(rename = "profile_image_url")]
    pub image_url: String,
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
    pub fn is_live(&self) -> bool {
        self.live
    }
}

fn get_live<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(<&str as Deserialize>::deserialize(d)? == "live")
}

#[derive(Deserialize)]
pub struct TwitchData<T> {
    pub data: Vec<T>,
}

// TODO: Serialize
#[derive(Debug, Deserialize)]
pub struct TwitchVideo {
    created_at: DateTime<Utc>,
    /// video duration in seconds
    #[serde(deserialize_with = "duration_to_u32")]
    duration: u32,
    #[serde(deserialize_with = "str_to_u64")]
    id: u64,
    published_at: DateTime<Utc>,
    title: String,
    url: String,
}

fn duration_to_u32<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let mut s: &str = Deserialize::deserialize(d)?;
    let mut seconds = 0;

    if let Some(idx) = s.find('h') {
        let hrs = s[..idx]
            .parse::<u32>()
            .map_err(|_| Error::custom(format!("failed to parse `{}` as hours", &s[..idx])))?;

        seconds += hrs * 3600;
        s = &s[idx + 1..];
    }

    if let Some(idx) = s.find('m') {
        let mins = s[..idx]
            .parse::<u32>()
            .map_err(|_| Error::custom(format!("failed to parse `{}` as minutes", &s[..idx])))?;

        seconds += mins * 60;
        s = &s[idx + 1..];
    }

    s = &s[..s.len() - 1];

    let secs = s
        .parse::<u32>()
        .map_err(|_| Error::custom(format!("failed to parse `{}` as seconds", s)))?;

    seconds += secs;

    Ok(seconds)
}
