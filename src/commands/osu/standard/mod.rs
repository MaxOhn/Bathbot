pub use crate::commands::osu::{
    common::*, nochoke::*, pp::*, profile::*, recent::*, simulate_recent::*, top::*, whatif::*,
};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s standard mode"]
#[commands(recent, top, profile, pp, whatif, simulaterecent, nochoke, common)]
pub struct Osu;
