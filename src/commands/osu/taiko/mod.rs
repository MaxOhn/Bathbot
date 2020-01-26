pub use crate::commands::osu::recent::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s taiko mode"]
#[commands(recenttaiko)]
pub struct Taiko;
