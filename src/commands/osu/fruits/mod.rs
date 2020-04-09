pub use crate::commands::osu::{
    common::*, pp::*, profile::*, rank::*, recent::*, recent_lb::*, top::*, whatif::*,
};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s ctb mode"]
#[commands(
    recentctb,
    topctb,
    recentbestctb,
    profilectb,
    ppctb,
    whatifctb,
    rankctb,
    commonctb,
    recentctbleaderboard,
    recentctbgloballeaderboard
)]
pub struct CatchTheBeat;
