use super::BgGames;
use crate::Context;

impl Context {
    pub fn bg_games(&self) -> &BgGames {
        &self.data.games.bg
    }
}
