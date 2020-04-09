mod ratios;

pub use ratios::*;

pub use crate::commands::osu::{
    common::*, pp::*, profile::*, rank::*, recent::*, recent_lb::*, simulate_recent::*, top::*,
    whatif::*,
};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s mania mode"]
#[commands(
    recentmania,
    topmania,
    recentbestmania,
    profilemania,
    ppmania,
    whatifmania,
    rankmania,
    commonmania,
    recentmanialeaderboard,
    recentmaniagloballeaderboard,
    simulaterecentmania,
    ratios
)]
pub struct Mania;
