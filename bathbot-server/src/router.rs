use std::sync::Arc;

use axum::{body::Body, http::Request, routing::get, Router};
use tower_http::trace::TraceLayer;
use tracing::Span;

use crate::{
    routes::{
        auth::{auth_css, auth_icon, osu::auth_osu, twitch::auth_twitch},
        guild_count::get_guild_count,
        metrics::get_metrics,
    },
    state::AppState,
};

pub fn create_router(state: AppState) -> Router {
    let trace = TraceLayer::new_for_http().on_request(|request: &Request<Body>, _span: &Span| {
        debug!("{} {}", request.method(), request.uri().path())
    });

    // TODO
    // let error_handler = ServiceBuilder::new().layer(HandleErrorLayer::new(test));

    // async fn test(err: BoxError) -> hyper::StatusCode {
    //     hyper::StatusCode::INTERNAL_SERVER_ERROR
    // }

    Router::new()
        .route("/metrics", get(get_metrics))
        .route("/guild_count", get(get_guild_count))
        .route("/auth/osu", get(auth_osu))
        .route("/auth/twitch", get(auth_twitch))
        .route("/auth/auth.css", get(auth_css))
        .route("/auth/icon.svg", get(auth_icon))
        // .route("/osudirect/:mapset_id", get(hello_world))
        // .layer(error_handler)
        .layer(trace)
        .with_state(Arc::new(state))
}
