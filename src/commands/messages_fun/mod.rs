mod hivemind;
mod impersonate;
mod stats;

pub use hivemind::*;
pub use impersonate::*;
pub use stats::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Babbling (random) sentences"]
#[commands(impersonate, hivemind, messagestats, randomhistory)]
struct MessagesFun;
