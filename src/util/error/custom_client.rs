use reqwest::Error as ReqwestError;
use serde_json::Error as SerdeJsonError;
use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum CustomClientError {
    MissingElement(&'static str),
    Parsing {
        body: String,
        source: SerdeJsonError,
        request: &'static str,
    },
    RankIndex(usize),
    RankNode(u8),
    Reqwest(ReqwestError),
    OsuStatsTimeout,
}

impl fmt::Display for CustomClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::MissingElement(element) => write!(f, "missing html element `{}`", element),
            Self::Parsing { request, body, .. } => {
                write!(f, "could not deserialize {}: {}", request, body)
            }
            Self::RankIndex(n) => write!(f, "expected rank between 1 and 10_000, got {}", n),
            Self::RankNode(n) => write!(f, "error at unwrap {}, expected  child", n),
            Self::Reqwest(_) => f.write_str("reqwest error"),
            Self::OsuStatsTimeout => f.write_str("timeout while waiting for osu stats"),
        }
    }
}

impl StdError for CustomClientError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::MissingElement(_) => None,
            Self::Parsing { source, .. } => Some(source),
            Self::RankIndex(_) => None,
            Self::RankNode(_) => None,
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
