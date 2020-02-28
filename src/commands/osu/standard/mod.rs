mod nochoke;

pub use nochoke::*;

pub use crate::commands::osu::{
    common::*, pp::*, profile::*, rank::*, recent::*, recent_lb::*, simulate_recent::*, top::*,
    whatif::*,
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
    common,
    recentleaderboard,
    rank,
    simulaterecent,
    nochoke
)]
pub struct Osu;
