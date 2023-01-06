use std::sync::Arc;

use axum::extract::State;
use eyre::Result;
use prometheus::{Encoder, TextEncoder};

use crate::{error::AppError, state::AppState};

pub async fn get_metrics(State(state): State<Arc<AppState>>) -> Result<Vec<u8>, AppError> {
    let mut buf = Vec::new();
    let encoder = TextEncoder::new();
    let metric_families = state.metrics.gather();
    encoder.encode(&metric_families, &mut buf)?;

    Ok(buf)
}
