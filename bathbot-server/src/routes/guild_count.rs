use std::sync::Arc;

use axum::{extract::State, Json};
use serde::Serialize;

use crate::state::AppState;

pub async fn get_guild_count(State(state): State<Arc<AppState>>) -> Json<GuildCount> {
    let guild_count = state.metrics.guild_counter.get();

    Json(GuildCount { guild_count })
}

#[derive(Serialize)]
pub struct GuildCount {
    guild_count: i64,
}
