use std::sync::Arc;

use axum::{extract::State, http::StatusCode};
use eyre::Result;
use prometheus::TextEncoder;

use crate::state::AppState;

pub async fn get_metrics(State(state): State<Arc<AppState>>) -> Result<String, StatusCode> {
    let metric_families = state.metrics.registry.gather();

    match TextEncoder::new().encode_to_string(&metric_families) {
        Ok(metrics) => Ok(metrics),
        Err(err) => {
            error!(?err, "Failed to encode metrics");

            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
