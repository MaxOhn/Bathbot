#![allow(non_upper_case_globals)]

use twilight_model::id::{marker::UserMarker, Id};

use crate::{commands::fun::GameDifficulty, error::BgGameError};

pub use self::{game_wrapper::GameWrapper, mapset::GameMapset, tags::MapsetTags};

mod game;
mod game_wrapper;
mod hints;
mod img_reveal;
mod mapset;
mod tags;
mod util;

pub mod components;

type GameResult<T> = Result<T, BgGameError>;

bitflags::bitflags! {
    pub struct Effects: u8 {
        const Blur           = 1 << 0;
        const Contrast       = 1 << 1;
        const FlipHorizontal = 1 << 2;
        const FlipVertical   = 1 << 3;
        const Grayscale      = 1 << 4;
        const Invert         = 1 << 5;
    }
}

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
