use serde_json::Error as SerdeJsonError;
use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum CustomClientError {
    DataUserId,
    SerdeLeaderboard(SerdeJsonError, String),
    SerdeMostPlayed(SerdeJsonError, String),
    SerdeSnipeCountry(SerdeJsonError, String),
    SerdeSnipeDifference(SerdeJsonError, String),
    SerdeSnipePlayer(SerdeJsonError, String),
    SerdeSnipeRecent(SerdeJsonError, String),
    SerdeSnipeScore(SerdeJsonError, String),
    RankIndex(usize),
    RankingPageTable,
    RankNode(u8),
    TBody,
}

impl fmt::Display for CustomClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::DataUserId => f.write_str("no attribute `data-user-id`"),
            Self::SerdeLeaderboard(e, content) => write!(
                f,
                "could not deserialize response for leaderboard: {}\n{}",
                e, content
            ),
            Self::SerdeMostPlayed(e, content) => write!(
                f,
                "could not deserialize response for most played: {}\n{}",
                e, content
            ),
            Self::SerdeSnipeCountry(e, content) => write!(
                f,
                "could not deserialize response for snipe country: {}\n{}",
                e, content
            ),
            Self::SerdeSnipeDifference(e, content) => write!(
                f,
                "could not deserialize response for snipe difference: {}\n{}",
                e, content
            ),
            Self::SerdeSnipePlayer(e, content) => write!(
                f,
                "could not deserialize response for snipe player: {}\n{}",
                e, content
            ),
            Self::SerdeSnipeRecent(e, content) => write!(
                f,
                "could not deserialize response for snipe recent: {}\n{}",
                e, content
            ),
            Self::SerdeSnipeScore(e, content) => write!(
                f,
                "could not deserialize response for snipe scores: {}\n{}",
                e, content
            ),
            Self::RankIndex(n) => write!(f, "expected rank between 1 and 10_000, got {}", n),
            Self::RankingPageTable => f.write_str("no class `ranking-page-table`"),
            Self::RankNode(n) => write!(f, "error at unwrap {}, expected  child", n),
            Self::TBody => f.write_str("no element `tbody`"),
        }
    }
}

impl StdError for CustomClientError {}
