use serde_json::Error as SerdeJsonError;
use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum CustomClientError {
    GlobalsList(SerdeJsonError, String),
    Leaderboard(SerdeJsonError, String),
    MissingElement(&'static str),
    MostPlayed(SerdeJsonError, String),
    RankIndex(usize),
    RankNode(u8),
    SnipeCountry(SerdeJsonError, String),
    SnipeDifference(SerdeJsonError, String),
    SnipePlayer(SerdeJsonError, String),
    SnipeRecent(SerdeJsonError, String),
    SnipeScore(SerdeJsonError, String),
}

impl fmt::Display for CustomClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::GlobalsList(_, c) => {
                write!(f, "could not deserialize globals list response: {}", c)
            }
            Self::Leaderboard(_, c) => {
                write!(f, "could not deserialize leaderboard response: {}", c)
            }
            Self::MissingElement(element) => write!(f, "missing html element `{}`", element),
            Self::MostPlayed(_, c) => {
                write!(f, "could not deserialize most player response: {}", c)
            }
            Self::RankIndex(n) => write!(f, "expected rank between 1 and 10_000, got {}", n),
            Self::RankNode(n) => write!(f, "error at unwrap {}, expected  child", n),
            Self::SnipeCountry(_, c) => {
                write!(f, "could not deserialize snipe country response: {}", c)
            }
            Self::SnipeDifference(_, c) => {
                write!(f, "could not deserialize snipe difference response: {}", c)
            }
            Self::SnipePlayer(_, c) => {
                write!(f, "could not deserialize snipe player response: {}", c)
            }
            Self::SnipeRecent(_, c) => {
                write!(f, "could not deserialize snipe recent response: {}", c)
            }
            Self::SnipeScore(_, c) => {
                write!(f, "could not deserialize snipe score response: {}", c)
            }
        }
    }
}

impl StdError for CustomClientError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::GlobalsList(e, _)
            | Self::Leaderboard(e, _)
            | Self::MostPlayed(e, _)
            | Self::SnipeCountry(e, _)
            | Self::SnipeDifference(e, _)
            | Self::SnipePlayer(e, _)
            | Self::SnipeRecent(e, _)
            | Self::SnipeScore(e, _) => Some(e),
            Self::MissingElement(_) => None,
            Self::RankIndex(_) => None,
            Self::RankNode(_) => None,
        }
    }
}
