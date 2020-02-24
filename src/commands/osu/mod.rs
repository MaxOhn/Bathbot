pub mod common;
pub mod fruits;
mod link;
pub mod mania;
pub mod nochoke;
pub mod pp;
pub mod profile;
pub mod recent;
pub mod recent_lb;
mod scores;
pub mod simulate_recent;
pub mod standard;
pub mod taiko;
pub mod top;
pub mod whatif;

pub use self::fruits::*;
pub use self::mania::*;
pub use self::standard::*;
pub use self::taiko::*;
pub use link::*;
pub use scores::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for all osu! modes"]
#[commands(scores, link)]
struct OsuGeneral;

pub const MINIMIZE_DELAY: i64 = 45;
