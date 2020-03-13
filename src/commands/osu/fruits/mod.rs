pub use crate::commands::osu::{
    common::*, pp::*, profile::*, rank::*, recent::*, recent_lb::*, recent_list::*, top::*,
    whatif::*,
};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s ctb mode"]
#[commands(
    recentctb,
    topctb,
    recentbestctb,
    recentlistctb,
    profilectb,
    ppctb,
    whatifctb,
    commonctb,
    recentctbleaderboard,
    recentctbgloballeaderboard,
    rankctb
)]
pub struct CatchTheBeat;
