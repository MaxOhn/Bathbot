use axum::{
    extract::rejection::QueryRejection,
    response::{IntoResponse, Response},
    Json,
};
use handlebars::RenderError;
use hyper::StatusCode;
use rosu_v2::prelude::OsuError;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
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
    #[error("prometheus error")]
    Prometheus(#[from] prometheus::Error),
    #[error("failed to await response bytes")]
    ResponseBytes(#[source] hyper::Error),
    #[error("failed to build twitch request")]
    TwitchRequest(#[from] axum::http::Error),
    #[error("failed to receive twitch response")]
    TwitchResponse(#[source] hyper::Error),
}

// TODO: use source
impl IntoResponse for AppError {
    #[inline]
    fn into_response(self) -> Response {
        let (code, msg) = match self {
            Self::BadAuthParams(_) => (StatusCode::BAD_REQUEST, "Insufficient query"),
            Self::DeserializeTwitch(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected response from twitch API",
            ),
            Self::Render(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
            Self::EmptyStandby => (StatusCode::BAD_REQUEST, "Invalid query code"),
            Self::EmptyTwitchData => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Received empty twitch response",
            ),
            Self::OsuApi(_) => (StatusCode::INTERNAL_SERVER_ERROR, "osu! API error"),
            Self::OsuAuthClient(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to authenticate user",
            ),
            Self::Prometheus(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
            Self::ResponseBytes(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
            Self::TwitchRequest(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
            Self::TwitchResponse(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
        };

        (code, Json(ErrorMessage { error: msg })).into_response()
    }
}

#[derive(Serialize)]
struct ErrorMessage {
    error: &'static str,
}
