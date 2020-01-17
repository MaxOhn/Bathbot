pub mod scores;
pub mod standard;

pub use self::standard::*;
pub use scores::*;

use serenity::framework::standard::macros::group;

#[group]
#[commands(scores)]
struct OsuGeneral;
