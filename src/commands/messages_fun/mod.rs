mod disable;
mod enable;
mod hivemind;
mod impersonate;
mod stats;

pub use disable::*;
pub use enable::*;
pub use hivemind::*;
pub use impersonate::*;
pub use stats::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Babbling (random) sentences"]
#[commands(
    enabletracking,
    disabletracking,
    impersonate,
    hivemind,
    messagestats,
    randomhistory
)]
struct MessagesFun;
