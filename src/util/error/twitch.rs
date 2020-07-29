use reqwest::{header::InvalidHeaderValue, Error as ReqwestError};
use serde_json::Error as SerdeJsonError;
use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum TwitchError {
    InvalidAuth(SerdeJsonError),
    InvalidHeader(InvalidHeaderValue),
    NoUserResult(String),
    Reqwest(ReqwestError),
    Serde(SerdeJsonError),
}

impl fmt::Display for TwitchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidAuth(e) => write!(f, "invalid auth response: {}", e),
            Self::InvalidHeader(e) => write!(f, "invalid client id: {}", e),
            Self::NoUserResult(n) => write!(f, "no result for name `{}`", n),
            Self::Reqwest(e) => write!(f, "reqwest error: {}", e),
            Self::Serde(e) => write!(f, "error while deserializing: {}", e),
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

impl From<SerdeJsonError> for TwitchError {
    fn from(e: SerdeJsonError) -> Self {
        Self::Serde(e)
    }
}

impl StdError for TwitchError {}
