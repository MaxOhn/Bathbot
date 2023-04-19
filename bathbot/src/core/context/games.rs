use super::{BgGames, HlGames, HlRetries};
use crate::Context;

impl Context {
    pub fn bg_games(&self) -> &BgGames {
        &self.data.games.bg
    }

    pub fn hl_games(&self) -> &HlGames {
        &self.data.games.hl
    }

    pub fn hl_retries(&self) -> &HlRetries {
        &self.data.games.hl_retries
    }
}
