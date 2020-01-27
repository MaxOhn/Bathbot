pub use crate::commands::osu::{recent::*, top::*};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s standard mode"]
#[commands(recent, top)]
pub struct Osu;
