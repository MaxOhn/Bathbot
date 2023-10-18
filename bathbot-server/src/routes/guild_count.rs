use std::{slice, sync::Arc};

use axum::{extract::State, Json};
use metrics::{Key, Label};
use serde::Serialize;

use crate::state::AppState;

static GUILDS_LABEL: Label = Label::from_static_parts("kind", "Guilds");

pub async fn get_guild_count(State(state): State<Arc<AppState>>) -> Json<GuildCount> {
    let key = Key::from_static_parts("bathbot.cache_entries", slice::from_ref(&GUILDS_LABEL));
    let guild_count = state.metrics_reader.gauge_value(&key) as u64;

    Json(GuildCount { guild_count })
}

#[derive(Serialize)]
pub struct GuildCount {
    guild_count: u64,
}
