pub mod addstream;
pub mod removestream;

pub use addstream::*;
pub use removestream::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for tracking Twitch and Mixer streams"]
#[commands(addstream, removestream)]
struct Streams;
