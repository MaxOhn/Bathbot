use thiserror::Error;

#[derive(Debug, Error)]
pub enum CustomClientError {
    #[error("could not deserialize {request}: {body}")]
    Parsing {
        body: String,
        request: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("reqwest error")]
    Reqwest(#[from] reqwest::Error),
    #[error("timeout while waiting for osu stats")]
    OsuStatsTimeout,
}
