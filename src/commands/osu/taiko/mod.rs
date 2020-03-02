pub use crate::commands::osu::{
    common::*, pp::*, profile::*, rank::*, recent::*, recent_lb::*, top::*, whatif::*,
};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s taiko mode"]
#[commands(
    recenttaiko,
    toptaiko,
    recentbesttaiko,
    profiletaiko,
    pptaiko,
    whatiftaiko,
    commontaiko,
    recenttaikoleaderboard,
    recenttaikogloballeaderboard,
    ranktaiko
)]
pub struct Taiko;
