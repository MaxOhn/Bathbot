use reqwest::{header::InvalidHeaderValue, Error as ReqwestError};
use serde_json::Error as SerdeJsonError;
use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum TwitchError {
    InvalidAuth(SerdeJsonError),
    InvalidHeader(InvalidHeaderValue),
    NoUserResult(String),
    Reqwest(ReqwestError),
    SerdeStreams(SerdeJsonError, String),
    SerdeUser(SerdeJsonError, String),
    SerdeUsers(SerdeJsonError, String),
}

impl fmt::Display for TwitchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidAuth(e) => write!(f, "invalid auth response: {}", e),
            Self::InvalidHeader(e) => write!(f, "invalid client id: {}", e),
            Self::NoUserResult(n) => write!(f, "no result for name `{}`", n),
            Self::Reqwest(e) => write!(f, "reqwest error: {}", e),
            Self::SerdeStreams(e, content) => write!(
                f,
                "could not deserialize response for streams: {}\n{}",
                e, content
            ),
            Self::SerdeUser(e, content) => write!(
                f,
                "could not deserialize response for user: {}\n{}",
                e, content
            ),
            Self::SerdeUsers(e, content) => write!(
                f,
                "could not deserialize response for users: {}\n{}",
                e, content
            ),
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

impl StdError for TwitchError {}
