mod command_count;
mod help;
mod ping;

pub use self::{command_count::*, help::*, ping::*};

use serenity::framework::standard::macros::group;

#[group]
#[commands(ping, commands)]
struct Utility;
