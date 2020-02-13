pub use crate::commands::osu::{
    nochoke::*, pp::*, profile::*, recent::*, simulate_recent::*, top::*, whatif::*,
};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s standard mode"]
#[commands(recent, top, profile, pp, whatif, simulaterecent, nochoke)]
pub struct Osu;
