#[macro_use]
mod macros;
pub mod commands;
pub mod util;

pub use commands::*;
pub use macros::*;

use rosu::backend::Osu as OsuClient;
use serenity::prelude::*;
use std::collections::HashMap;

pub struct CommandCounter;

impl TypeMapKey for CommandCounter {
    type Value = HashMap<String, u64>;
}

pub struct Osu;

impl TypeMapKey for Osu {
    type Value = OsuClient;
}
