pub use crate::commands::osu::{
    common::*, pp::*, profile::*, rank::*, recent::*, recent_lb::*, recent_list::*, top::*,
    whatif::*,
};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s taiko mode"]
#[commands(
    recenttaiko,
    toptaiko,
    recentbesttaiko,
    recentlisttaiko,
    profiletaiko,
    pptaiko,
    whatiftaiko,
    commontaiko,
    recenttaikoleaderboard,
    recenttaikogloballeaderboard,
    ranktaiko
)]
pub struct Taiko;
