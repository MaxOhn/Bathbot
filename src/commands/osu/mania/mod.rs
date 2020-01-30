pub use crate::commands::osu::{pp::*, profile::*, recent::*, top::*};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s mania mode"]
#[commands(recentmania, topmania, profilemania, ppmania)]
pub struct Mania;
