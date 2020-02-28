pub mod addstream;

pub use addstream::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for tracking Twitch and Mixer streams"]
#[commands(addstream)]
struct Streams;
