use std::{sync::Arc, time::Instant};

use axum::{
    extract::{MatchedPath, State},
    middleware::Next,
    response::Response,
};
use hyper::Request;

use crate::state::AppState;

pub async fn track_metrics<B>(
    State(state): State<Arc<AppState>>,
    req: Request<B>,
    next: Next<B>,
) -> Response {
    let path = match req.extensions().get::<MatchedPath>() {
        Some(matched_path) => matched_path.as_str().to_owned(),
        None => req.uri().path().to_owned(),
    };

    let method = req.method().to_string();

    let start = Instant::now();
    let response = next.run(req).await;
    let latency = start.elapsed().as_secs_f64();

    let status = response.status().as_u16().to_string();

    let labels = [method.as_str(), path.as_str(), status.as_str()];

    state.metrics.request_count.with_label_values(&labels).inc();
    state
        .metrics
        .response_time
        .with_label_values(&labels)
        .observe(latency);

    response
}
