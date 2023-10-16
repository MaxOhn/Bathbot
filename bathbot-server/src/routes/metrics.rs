use std::sync::Arc;

use axum::{extract::State, http::StatusCode};
use eyre::Result;

use crate::state::AppState;

pub async fn get_metrics(State(state): State<Arc<AppState>>) -> Result<String, StatusCode> {
    Ok(state.prometheus.render())
}
