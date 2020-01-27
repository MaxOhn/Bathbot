pub use crate::commands::osu::{recent::*, top::*};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s mania mode"]
#[commands(recentmania, topmania)]
pub struct Mania;
