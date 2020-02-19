pub use crate::commands::osu::{common::*, pp::*, profile::*, recent::*, top::*, whatif::*};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s taiko mode"]
#[commands(recenttaiko, toptaiko, profiletaiko, pptaiko, whatiftaiko, commontaiko)]
pub struct Taiko;
