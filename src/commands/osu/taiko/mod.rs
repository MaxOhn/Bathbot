pub use crate::commands::osu::{pp::*, profile::*, recent::*, top::*};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s taiko mode"]
#[commands(recenttaiko, toptaiko, profiletaiko, pptaiko)]
pub struct Taiko;
