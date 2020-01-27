pub use crate::commands::osu::{recent::*, top::*};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s ctb mode"]
#[commands(recentctb, topctb)]
pub struct CatchTheBeat;
