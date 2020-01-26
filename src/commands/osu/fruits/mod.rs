pub use crate::commands::osu::recent::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s ctb mode"]
#[commands(recentctb)]
pub struct CatchTheBeat;
