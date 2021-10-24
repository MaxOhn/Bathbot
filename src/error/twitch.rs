use serde_json::Error as SerdeJsonError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TwitchError {
    #[error("hyper error")]
    Hyper(#[from] hyper::Error),
    #[error("invalid client id")]
    InvalidHeader(#[from] reqwest::header::InvalidHeaderValue),
    #[error("no user provided by api after authorization")]
    NoUser,
    #[error("reqwest error")]
    Reqwest(#[from] reqwest::Error),
    #[error("could not deserialize response for streams: {content}")]
    SerdeStreams {
        #[source]
        source: SerdeJsonError,
        content: String,
    },
    #[error("could not deserialize response for token: {content}")]
    SerdeToken {
        #[source]
        source: SerdeJsonError,
        content: String,
    },
    #[error("could not deserialize response for user: {content}")]
    SerdeUser {
        #[source]
        source: SerdeJsonError,
        content: String,
    },
    #[error("could not deserialize response for users: {content}")]
    SerdeUsers {
        #[source]
        source: SerdeJsonError,
        content: String,
    },
    #[error("could not deserialize response for videos: {content}")]
    SerdeVideos {
        #[source]
        source: SerdeJsonError,
        content: String,
    },
}
