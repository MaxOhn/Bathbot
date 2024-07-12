use std::time::Instant;

use axum::{extract::MatchedPath, middleware::Next, response::Response};
use hyper::Request;
use metrics::histogram;

pub async fn track_metrics<B>(req: Request<B>, next: Next<B>) -> Response {
    let path = match req.extensions().get::<MatchedPath>() {
        Some(matched_path) => matched_path.as_str().to_owned(),
        None => req.uri().path().to_owned(),
    };

    let method = req.method().to_string();

    let start = Instant::now();
    let response = next.run(req).await;
    let latency = start.elapsed();
    let status = response.status().as_str().to_string();

    histogram!(
        "server_response_time",
        "method" => method,
        "path" => path,
        "status" => status
    )
    .record(latency);

    response
}
