use hyper::Error as HyperError;
use reqwest::{header::InvalidHeaderValue, Error as ReqwestError};
use serde_json::Error as SerdeJsonError;
use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum TwitchError {
    Hyper(HyperError),
    InvalidHeader(InvalidHeaderValue),
    NoUser,
    Reqwest(ReqwestError),
    SerdeStreams {
        source: SerdeJsonError,
        content: String,
    },
    SerdeToken {
        source: SerdeJsonError,
        content: String,
    },
    SerdeUser {
        source: SerdeJsonError,
        content: String,
    },
    SerdeUsers {
        source: SerdeJsonError,
        content: String,
    },
    SerdeVideos {
        source: SerdeJsonError,
        content: String,
    },
}

impl fmt::Display for TwitchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Hyper(_) => f.write_str("hyper error"),
            Self::InvalidHeader(_) => f.write_str("invalid client id"),
            Self::NoUser => f.write_str("no user provided by api after authorization"),
            Self::Reqwest(_) => f.write_str("reqwest error"),
            Self::SerdeStreams { content, .. } => {
                write!(f, "could not deserialize response for streams: {}", content)
            }
            Self::SerdeToken { content, .. } => {
                write!(f, "could not deserialize response for token: {}", content)
            }
            Self::SerdeUser { content, .. } => {
                write!(f, "could not deserialize response for user: {}", content)
            }
            Self::SerdeUsers { content, .. } => {
                write!(f, "could not deserialize response for users: {}", content)
            }
            Self::SerdeVideos { content, .. } => {
                write!(f, "could not deserialize response for videos: {}", content)
            }
        }
    }
}

impl StdError for TwitchError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Hyper(e) => Some(e),
            Self::InvalidHeader(e) => Some(e),
            Self::NoUser => None,
            Self::Reqwest(e) => Some(e),
            Self::SerdeStreams { source, .. } => Some(source),
            Self::SerdeToken { source, .. } => Some(source),
            Self::SerdeUser { source, .. } => Some(source),
            Self::SerdeUsers { source, .. } => Some(source),
            Self::SerdeVideos { source, .. } => Some(source),
        }
    }
}

impl From<HyperError> for TwitchError {
    fn from(e: HyperError) -> Self {
        Self::Hyper(e)
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
