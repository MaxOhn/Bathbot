pub use crate::commands::osu::{
    common::*, pp::*, profile::*, rank::*, recent::*, recent_lb::*, top::*, whatif::*,
};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s taiko mode"]
#[commands(
    recenttaiko,
    toptaiko,
    profiletaiko,
    pptaiko,
    whatiftaiko,
    commontaiko,
    recenttaikoleaderboard,
    ranktaiko
)]
pub struct Taiko;
