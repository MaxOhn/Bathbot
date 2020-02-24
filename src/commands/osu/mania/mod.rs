mod ratios;

pub use ratios::*;

pub use crate::commands::osu::{
    common::*, pp::*, profile::*, recent::*, recent_lb::*, top::*, whatif::*,
};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s mania mode"]
#[commands(
    recentmania,
    topmania,
    profilemania,
    ppmania,
    whatifmania,
    commonmania,
    ratios,
    recentmanialeaderboard
)]
pub struct Mania;
