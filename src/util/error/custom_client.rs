use reqwest::Error as ReqwestError;
use serde_json::Error as SerdeJsonError;
use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum CustomClientError {
    Parsing {
        body: String,
        source: SerdeJsonError,
        request: &'static str,
    },
    Reqwest(ReqwestError),
    OsuStatsTimeout,
}

impl fmt::Display for CustomClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Parsing { request, body, .. } => {
                write!(f, "could not deserialize {}: {}", request, body)
            }
            Self::Reqwest(_) => f.write_str("reqwest error"),
            Self::OsuStatsTimeout => f.write_str("timeout while waiting for osu stats"),
        }
    }
}

impl StdError for CustomClientError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Parsing { source, .. } => Some(source),
            Self::Reqwest(e) => Some(e),
            Self::OsuStatsTimeout => None,
        }
    }
}

impl From<ReqwestError> for CustomClientError {
    fn from(error: ReqwestError) -> Self {
        Self::Reqwest(error)
    }
}
