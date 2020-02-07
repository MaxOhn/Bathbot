mod command_count;
mod help;
mod ping;
mod prune;

pub use self::{command_count::*, help::*, ping::*, prune::*};

use serenity::framework::standard::macros::group;

#[group]
#[commands(ping, commands, prune)]
struct Utility;
