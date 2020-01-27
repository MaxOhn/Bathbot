pub mod fruits;
pub mod mania;
pub mod recent;
pub mod scores;
pub mod standard;
pub mod taiko;
pub mod top;

pub use self::fruits::*;
pub use self::mania::*;
pub use self::standard::*;
pub use self::taiko::*;
pub use scores::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for all osu! modes"]
#[commands(scores)]
struct OsuGeneral;
