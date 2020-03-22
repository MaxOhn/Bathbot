mod bg_game;
mod hivemind;
mod impersonate;
mod minesweeper;
mod songs;

pub use bg_game::*;
pub use hivemind::*;
pub use impersonate::*;
pub use minesweeper::*;
pub use songs::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Random fun commands"]
#[commands(
    backgroundgame,
    minesweeper,
    impersonate,
    hivemind,
    bombsaway,
    catchit,
    ding,
    fireandflames,
    flamingo,
    pretender,
    rockefeller,
    tijdmachine
)]
struct Fun;
