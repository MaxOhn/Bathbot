use serde_json::Error as SerdeJsonError;
use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum CustomClientError {
    DataUserId,
    RankIndex(usize),
    RankingPageTable,
    RankNode(u8),
    Request(RequestError),
    TBody,
}

#[derive(Debug)]
enum RequestType {
    GlobalsList,
    Leaderboard,
    MostPlayed,
    SnipeCountry,
    SnipeDifference,
    SnipePlayer,
    SnipeRecent,
    SnipeScore,
}

#[derive(Debug)]
pub struct RequestError {
    request: RequestType,
    error: SerdeJsonError,
    content: String,
}

impl CustomClientError {
    pub fn globals_list(error: SerdeJsonError, content: String) -> Self {
        Self::Request(RequestError {
            request: RequestType::GlobalsList,
            error,
            content,
        })
    }
    pub fn leaderboard(error: SerdeJsonError, content: String) -> Self {
        Self::Request(RequestError {
            request: RequestType::Leaderboard,
            error,
            content,
        })
    }
    pub fn most_played(error: SerdeJsonError, content: String) -> Self {
        Self::Request(RequestError {
            request: RequestType::MostPlayed,
            error,
            content,
        })
    }
    pub fn snipe_country(error: SerdeJsonError, content: String) -> Self {
        Self::Request(RequestError {
            request: RequestType::SnipeCountry,
            error,
            content,
        })
    }
    pub fn snipe_difference(error: SerdeJsonError, content: String) -> Self {
        Self::Request(RequestError {
            request: RequestType::SnipeDifference,
            error,
            content,
        })
    }
    pub fn snipe_recent(error: SerdeJsonError, content: String) -> Self {
        Self::Request(RequestError {
            request: RequestType::SnipeRecent,
            error,
            content,
        })
    }
    pub fn snipe_score(error: SerdeJsonError, content: String) -> Self {
        Self::Request(RequestError {
            request: RequestType::SnipeScore,
            error,
            content,
        })
    }
    pub fn snipe_player(error: SerdeJsonError, content: String) -> Self {
        Self::Request(RequestError {
            request: RequestType::SnipePlayer,
            error,
            content,
        })
    }
}

impl fmt::Display for CustomClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::DataUserId => f.write_str("no attribute `data-user-id`"),
            Self::Request(RequestError {
                request,
                error,
                content,
            }) => write!(
                f,
                "could not deserialize response for {}: {}\n{}",
                match request {
                    RequestType::GlobalsList => "globals list",
                    RequestType::Leaderboard => "leaderboard",
                    RequestType::MostPlayed => "most played",
                    RequestType::SnipeCountry => "snipe country",
                    RequestType::SnipeDifference => "snipe difference",
                    RequestType::SnipePlayer => "snipe player",
                    RequestType::SnipeRecent => "snipe recent",
                    RequestType::SnipeScore => "snipe score",
                },
                error,
                content
            ),
            Self::RankIndex(n) => write!(f, "expected rank between 1 and 10_000, got {}", n),
            Self::RankingPageTable => f.write_str("no class `ranking-page-table`"),
            Self::RankNode(n) => write!(f, "error at unwrap {}, expected  child", n),
            Self::TBody => f.write_str("no element `tbody`"),
        }
    }
}

impl StdError for CustomClientError {}
