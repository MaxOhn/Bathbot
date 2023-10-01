use std::{fmt, fmt::Write};

use http::HeaderValue;
use serde::{de::Error, Deserialize, Deserializer};
use time::{Duration, OffsetDateTime};

use crate::deser::datetime_z;

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

pub struct TwitchData {
    pub client_id: HeaderValue,
    pub oauth_token: TwitchOAuthToken,
}

#[derive(Default, Deserialize)]
pub struct TwitchOAuthToken {
    access_token: Box<str>,
}

impl fmt::Display for TwitchOAuthToken {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.access_token)
    }
}

#[derive(Debug, Deserialize)]
pub struct TwitchUser {
    #[serde(rename = "description")]
    pub bio: Box<str>,
    #[serde(rename = "login")]
    pub display_name: Box<str>,
    #[serde(rename = "profile_image_url")]
    pub image_url: Box<str>,
    #[serde(rename = "id", deserialize_with = "str_to_u64")]
    pub user_id: u64,
}

#[derive(Deserialize, Debug)]
pub struct TwitchStream {
    #[serde(rename = "game_id", deserialize_with = "str_to_maybe_u64")]
    pub game_id: Option<u64>,
    #[serde(rename = "id", deserialize_with = "str_to_u64")]
    pub stream_id: u64,
    // Gets modified inside the struct so required to keep as `String`
    pub thumbnail_url: String,
    pub title: Box<str>,
    #[serde(deserialize_with = "str_to_u64")]
    pub user_id: u64,
    #[serde(rename = "user_login")]
    pub login: Box<str>,
    #[serde(rename = "user_name")]
    pub username: Box<str>,
    #[serde(rename = "type", deserialize_with = "get_live")]
    pub live: bool,
    #[serde(with = "datetime_z")]
    pub started_at: OffsetDateTime,
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
pub struct TwitchDataList<T> {
    pub data: Vec<T>,
}

#[derive(Debug, Deserialize)]
pub struct TwitchVideo {
    #[serde(with = "datetime_z")]
    pub created_at: OffsetDateTime,
    /// Video duration in seconds
    #[serde(deserialize_with = "duration_to_u32")]
    pub duration: u32,
    #[serde(deserialize_with = "str_to_u64")]
    pub id: u64,
    #[serde(with = "datetime_z")]
    pub published_at: OffsetDateTime,
    pub title: Box<str>,
    // Gets modified inside the struct so required to keep as `String`
    pub url: String,
    #[serde(rename = "user_name")]
    pub username: Box<str>,
    #[serde(rename = "user_login")]
    pub login: Box<str>,
}

impl TwitchVideo {
    pub fn ended_at(&self) -> OffsetDateTime {
        self.created_at + Duration::seconds(self.duration as i64)
    }

    pub fn append_url_timestamp(url: &mut String, offset: Duration) {
        let mut offset = offset.whole_seconds();

        url.push_str("?t=");

        if offset >= 3600 {
            let _ = write!(url, "{}h", offset / 3600);
            offset %= 3600;
        }

        if offset >= 60 {
            let _ = write!(url, "{}m", offset / 60);
            offset %= 60;
        }

        if offset > 0 {
            let _ = write!(url, "{offset}s");
        }
    }
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
        .map_err(|_| Error::custom(format!("failed to parse `{s}` as seconds")))?;

    seconds += secs;

    Ok(seconds)
}
