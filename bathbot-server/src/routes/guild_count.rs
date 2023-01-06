use std::sync::Arc;

use axum::extract::State;

use crate::state::AppState;

pub async fn get_guild_count(State(state): State<Arc<AppState>>) -> String {
    42.to_string()
}
