use bathbot_model::{Effects, MapsetTags};
use twilight_model::id::{marker::UserMarker, Id};

use crate::commands::fun::GameDifficulty;

pub use self::{game_wrapper::GameWrapper, mapset::GameMapset};

mod game;
mod game_wrapper;
mod hints;
mod img_reveal;
mod mapset;
mod util;

pub mod components;

#[derive(Clone)]
pub enum GameState {
    Running {
        game: GameWrapper,
    },
    Setup {
        author: Id<UserMarker>,
        difficulty: GameDifficulty,
        effects: Effects,
        excluded: MapsetTags,
        included: MapsetTags,
    },
}
