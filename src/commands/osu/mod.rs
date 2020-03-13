pub mod common;
pub mod fruits;
mod leaderboard;
mod link;
pub mod mania;
mod match_costs;
pub mod pp;
pub mod profile;
pub mod rank;
pub mod recent;
pub mod recent_lb;
pub mod recent_list;
mod scores;
mod simulate;
pub mod simulate_recent;
pub mod standard;
pub mod taiko;
pub mod top;
pub mod whatif;

pub use self::fruits::*;
pub use self::mania::*;
pub use self::standard::*;
pub use self::taiko::*;
pub use leaderboard::*;
pub use link::*;
pub use match_costs::*;
pub use scores::*;
pub use simulate::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for all osu! modes"]
#[commands(link, scores, simulate, matchcosts, leaderboard, globalleaderboard)]
struct OsuGeneral;
