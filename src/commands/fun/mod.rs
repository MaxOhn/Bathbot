mod bg_game;

pub use bg_game::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Random fun commands"]
#[commands(backgroundgame)]
struct Fun;
