mod bg_game;
mod minesweeper;
mod songs;

pub use bg_game::*;
pub use minesweeper::*;
pub use songs::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Random fun commands"]
#[commands(
    backgroundgame,
    minesweeper,
    bombsaway,
    catchit,
    ding,
    fireandflames,
    flamingo,
    pretender,
    rockefeller,
    tijdmachine,
    bgtags
)]
struct Fun;
