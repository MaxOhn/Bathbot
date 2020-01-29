pub use crate::commands::osu::{profile::*, recent::*, top::*, pp::*};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s standard mode"]
#[commands(recent, top, profile, pp)]
pub struct Osu;
