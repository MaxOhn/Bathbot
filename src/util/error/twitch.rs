use reqwest::{header::InvalidHeaderValue, Error as ReqwestError};
use serde_json::Error as SerdeJsonError;
use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum TwitchError {
    InvalidAuth(SerdeJsonError),
    InvalidHeader(InvalidHeaderValue),
    Reqwest(ReqwestError),
    SerdeStreams(SerdeJsonError, String),
    SerdeUser(SerdeJsonError, String),
    SerdeUsers(SerdeJsonError, String),
    SerdeVideos(SerdeJsonError, String),
}

impl fmt::Display for TwitchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidAuth(_) => f.write_str("invalid auth response"),
            Self::InvalidHeader(_) => f.write_str("invalid client id"),
            Self::Reqwest(_) => f.write_str("reqwest error"),
            Self::SerdeStreams(_, content) => {
                write!(f, "could not deserialize response for streams: {}", content)
            }
            Self::SerdeUser(_, content) => {
                write!(f, "could not deserialize response for user: {}", content)
            }
            Self::SerdeUsers(_, content) => {
                write!(f, "could not deserialize response for users: {}", content)
            }
            Self::SerdeVideos(_, content) => {
                write!(f, "could not deserialize response for videos: {}", content)
            }
        }
    }
}

impl From<InvalidHeaderValue> for TwitchError {
    fn from(e: InvalidHeaderValue) -> Self {
        Self::InvalidHeader(e)
    }
}

impl From<ReqwestError> for TwitchError {
    fn from(e: ReqwestError) -> Self {
        Self::Reqwest(e)
    }
}

impl StdError for TwitchError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::InvalidAuth(e) => Some(e),
            Self::InvalidHeader(e) => Some(e),
            Self::Reqwest(e) => Some(e),
            Self::SerdeStreams(e, _) => Some(e),
            Self::SerdeUser(e, _) => Some(e),
            Self::SerdeUsers(e, _) => Some(e),
            Self::SerdeVideos(e, _) => Some(e),
        }
    }
}
