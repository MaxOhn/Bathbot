mod bg_game;
mod songs;

pub use bg_game::*;
pub use songs::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Random fun commands"]
#[commands(
    backgroundgame,
    bombsaway,
    catchit,
    ding,
    fireandflames,
    fireflies,
    flamingo,
    pretender,
    rockefeller,
    tijdmachine
)]
struct Fun;
