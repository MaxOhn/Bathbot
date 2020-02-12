pub mod fruits;
mod link;
pub mod mania;
pub mod pp;
pub mod profile;
pub mod recent;
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
use std::time::Duration;

#[group]
#[description = "Commands for all osu! modes"]
#[commands(scores, link)]
struct OsuGeneral;

pub const MINIMIZE_DELAY: Duration = Duration::from_secs(45);
