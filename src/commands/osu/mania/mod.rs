pub use crate::commands::osu::recent::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s mania mode"]
#[commands(recentmania)]
pub struct Mania;
