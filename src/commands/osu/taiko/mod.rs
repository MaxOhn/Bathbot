pub use crate::commands::osu::{
    common::*, osustats_globals::*, pp::*, profile::*, rank::*, recent::*, recent_lb::*, top::*,
    whatif::*,
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
    ranktaiko,
    commontaiko,
    recenttaikoleaderboard,
    recenttaikogloballeaderboard,
    osustatsglobalstaiko
)]
pub struct Taiko;
