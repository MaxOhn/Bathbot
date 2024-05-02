use super::BgGames;
use crate::Context;

impl Context {
    pub fn bg_games() -> &'static BgGames {
        &Context::get().data.games.bg
    }
}
