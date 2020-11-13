use serde_json::Error as SerdeJsonError;
use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum CustomClientError {
    MissingElement(&'static str),
    RankIndex(usize),
    RankNode(u8),
    Request(RequestError),
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

impl RequestType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::GlobalsList => "globals list",
            Self::Leaderboard => "leaderboard",
            Self::MostPlayed => "most played",
            Self::SnipeCountry => "snipe country",
            Self::SnipeDifference => "snipe difference",
            Self::SnipePlayer => "snipe player",
            Self::SnipeRecent => "snipe recent",
            Self::SnipeScore => "snipe score",
        }
    }
}

#[derive(Debug)]
pub struct RequestError {
    request: RequestType,
    error: SerdeJsonError,
    content: String,
}

impl StdError for RequestError {
    // fn source(&self) -> Option<&(dyn StdError + 'static)> {
    //     Some(&self.error)
    // }
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "type: {} | content: {}",
            self.request.as_str(),
            self.content
        )
    }
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
            Self::MissingElement(element) => write!(f, "missing html element `{}`", element),
            Self::Request(_) => f.write_str("could not deserialize response"),
            Self::RankIndex(n) => write!(f, "expected rank between 1 and 10_000, got {}", n),
            Self::RankNode(n) => write!(f, "error at unwrap {}, expected  child", n),
        }
    }
}

impl StdError for CustomClientError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::MissingElement(_) => None,
            Self::RankIndex(_) => None,
            Self::RankNode(_) => None,
            Self::Request(e) => Some(e),
        }
    }
}
