mod game;
mod game_wrapper;
mod hints;
mod img_reveal;
mod tags;
mod util;

use game::{game_loop, Game, LoopResult};
pub use game_wrapper::GameWrapper;
use hints::Hints;
use img_reveal::ImageReveal;
pub use tags::MapsetTags;

use crate::util::error::BgGameError;

type GameResult<T> = Result<T, BgGameError>;
