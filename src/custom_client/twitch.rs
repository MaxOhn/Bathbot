use std::fmt;

use serde::{de::Error, Deserialize, Deserializer};
use time::OffsetDateTime;

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

#[derive(Default, Deserialize)]
pub struct TwitchOAuthToken {
    access_token: String,
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
    pub bio: String,
    #[serde(rename = "login")]
    pub display_name: String,
    #[serde(rename = "profile_image_url")]
    pub image_url: String,
    #[serde(rename = "id", deserialize_with = "str_to_u64")]
    pub user_id: u64,
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
pub struct TwitchDataList<T> {
    pub data: Vec<T>,
}

#[derive(Debug, Deserialize)]
pub struct TwitchVideo {
    #[serde(with = "datetime")]
    pub created_at: OffsetDateTime,
    /// video duration in seconds
    #[serde(deserialize_with = "duration_to_u32")]
    pub duration: u32,
    #[serde(deserialize_with = "str_to_u64")]
    pub id: u64,
    #[serde(with = "datetime")]
    pub published_at: OffsetDateTime,
    pub title: String,
    pub url: String,
    #[serde(rename = "user_name")]
    pub username: String,
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

pub(super) mod datetime {
    use std::fmt;

    use serde::{
        de::{Error, Visitor},
        Deserializer,
    };
    use time::{format_description::FormatItem, OffsetDateTime, PrimitiveDateTime};

    use crate::util::datetime::{DATE_FORMAT, TIME_FORMAT};

    pub const DATETIMEZ_FORMAT: &[FormatItem<'_>] = &[
        FormatItem::Compound(DATE_FORMAT),
        FormatItem::Literal(b"T"),
        FormatItem::Compound(TIME_FORMAT),
        FormatItem::Literal(b"Z"),
    ];

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<OffsetDateTime, D::Error> {
        d.deserialize_str(DateTimeVisitor)
    }

    pub(super) struct DateTimeVisitor;

    impl<'de> Visitor<'de> for DateTimeVisitor {
        type Value = OffsetDateTime;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an `OffsetDateTime`")
        }

        #[inline]
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            PrimitiveDateTime::parse(v, DATETIMEZ_FORMAT)
                .map(PrimitiveDateTime::assume_utc)
                .map_err(Error::custom)
        }
    }
}
