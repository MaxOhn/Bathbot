use dashmap::DashMap;
use twilight_model::id::{
    marker::{ChannelMarker, UserMarker},
    Id,
};

use crate::{commands::fun::HlGameState, games::bg::GameState, Context};

impl Context {
    pub fn bg_games(&self) -> &DashMap<Id<ChannelMarker>, GameState> {
        &self.data.bg_games
    }

    pub fn hl_games(&self) -> &DashMap<Id<UserMarker>, HlGameState> {
        &self.data.hl_games
    }
}
