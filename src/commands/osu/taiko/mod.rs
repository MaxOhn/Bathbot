pub use crate::commands::osu::{profile::*, recent::*, top::*};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s taiko mode"]
#[commands(recenttaiko, toptaiko, profiletaiko)]
pub struct Taiko;
