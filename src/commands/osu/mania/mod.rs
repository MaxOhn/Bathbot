pub use crate::commands::osu::{profile::*, recent::*, top::*};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s mania mode"]
#[commands(recentmania, topmania, profilemania)]
pub struct Mania;
