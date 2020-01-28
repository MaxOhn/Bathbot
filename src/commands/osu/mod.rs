pub mod fruits;
pub mod mania;
pub mod profile;
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
use std::time::Duration;

#[group]
#[description = "Commands for all osu! modes"]
#[commands(scores)]
struct OsuGeneral;

pub const MINIMIZE_DELAY: Duration = Duration::from_millis(45_000);
