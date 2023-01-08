use axum::extract::rejection::QueryRejection;
use handlebars::RenderError;
use hyper::StatusCode;
use rosu_v2::prelude::OsuError;

#[derive(Debug, thiserror::Error)]
#[error("authentication error")]
pub enum AuthError {
    #[error("bad auth params")]
    BadAuthParams(#[from] QueryRejection),
    #[error("failed to deserialize twitch response")]
    DeserializeTwitch(serde_json::Error),
    #[error("failed to render page")]
    Render(#[from] RenderError),
    #[error("attempted to authenticate without an awaiting receiver")]
    EmptyStandby,
    #[error("received empty twitch data")]
    EmptyTwitchData,
    #[error("osu api error")]
    OsuApi(#[source] OsuError),
    #[error("failed to build authenticated osu client")]
    OsuAuthClient(#[source] OsuError),
    #[error("failed to await response bytes")]
    ResponseBytes(#[source] hyper::Error),
    #[error("failed to build twitch request")]
    TwitchRequest(#[from] axum::http::Error),
    #[error("failed to receive twitch response")]
    TwitchResponse(#[source] hyper::Error),
}

impl AuthError {
    pub fn response(&self) -> (StatusCode, &'static str) {
        match self {
            Self::BadAuthParams(_) => (StatusCode::BAD_REQUEST, "Insufficient query"),
            Self::DeserializeTwitch(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected response from twitch API",
            ),
            Self::Render(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
            Self::EmptyStandby => (StatusCode::BAD_REQUEST, "Unexpected authentication attempt"),
            Self::EmptyTwitchData => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Received empty twitch response",
            ),
            Self::OsuApi(_) => (StatusCode::INTERNAL_SERVER_ERROR, "osu! API error"),
            Self::OsuAuthClient(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to authenticate user",
            ),
            Self::ResponseBytes(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
            Self::TwitchRequest(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
            Self::TwitchResponse(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
        }
    }
}
