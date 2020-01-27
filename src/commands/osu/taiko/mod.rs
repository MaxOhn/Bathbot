pub use crate::commands::osu::{recent::*, top::*};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s taiko mode"]
#[commands(recenttaiko, toptaiko)]
pub struct Taiko;
