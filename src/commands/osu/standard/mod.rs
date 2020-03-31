mod nochoke;

pub use nochoke::*;

pub use crate::commands::osu::{
    common::*, pp::*, profile::*, rank::*, recent::*, recent_lb::*, recent_list::*,
    simulate_recent::*, top::*, whatif::*,
};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s standard mode"]
#[commands(
    recent,
    top,
    recentbest,
    recentlist,
    profile,
    pp,
    whatif,
    rank,
    common,
    recentleaderboard,
    recentgloballeaderboard,
    simulaterecent,
    nochokes,
    sotarks
)]
pub struct Osu;
