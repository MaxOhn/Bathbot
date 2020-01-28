pub use crate::commands::osu::{profile::*, recent::*, top::*};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s standard mode"]
#[commands(recent, top, profile)]
pub struct Osu;
