pub use crate::commands::osu::{
    common::*, nochoke::*, pp::*, profile::*, rank::*, recent::*, recent_lb::*, simulate_recent::*,
    top::*, whatif::*,
};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s standard mode"]
#[commands(
    recent,
    top,
    profile,
    pp,
    whatif,
    simulaterecent,
    common,
    recentleaderboard,
    rank,
    nochoke
)]
pub struct Osu;
