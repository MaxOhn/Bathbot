pub mod recent;

pub use self::recent::*;

use serenity::framework::standard::macros::group;

#[group]
#[commands(recent)]
struct OsuStd;
