mod nochoke;

pub use nochoke::*;

pub use crate::commands::osu::{
    common::*, osustats_globals::*, pp::*, profile::*, rank::*, recent::*, recent_lb::*,
    simulate_recent::*, top::*, whatif::*,
};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s standard mode"]
#[commands(
    recent,
    top,
    recentbest,
    profile,
    pp,
    whatif,
    rank,
    common,
    recentleaderboard,
    recentgloballeaderboard,
    osustatsglobals,
    simulaterecent,
    nochokes,
    sotarks
)]
pub struct Osu;
