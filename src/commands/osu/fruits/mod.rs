pub use crate::commands::osu::{pp::*, profile::*, recent::*, top::*};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for osu!'s ctb mode"]
#[commands(recentctb, topctb, profilectb, ppctb)]
pub struct CatchTheBeat;
