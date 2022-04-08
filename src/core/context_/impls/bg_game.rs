use dashmap::DashMap;
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::{commands::fun::GameState, Context};

// TODO: Refactor file
impl Context {
    pub fn bg_games(&self) -> &DashMap<Id<ChannelMarker>, GameState> {
        &self.data.bg_games
    }
}
