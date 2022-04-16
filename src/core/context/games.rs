use dashmap::DashMap;
use twilight_model::id::{
    marker::{ChannelMarker, UserMarker},
    Id,
};

use crate::{
    games::{bg::GameState as BgGameState, hl::GameState as HlGameState},
    Context,
};

impl Context {
    pub fn bg_games(&self) -> &DashMap<Id<ChannelMarker>, BgGameState> {
        &self.data.bg_games
    }

    pub fn hl_games(&self) -> &DashMap<Id<UserMarker>, HlGameState> {
        &self.data.hl_games
    }
}
