pub mod addstream;
pub mod allstreams;
pub mod removestream;
pub mod tracked;

pub use addstream::*;
pub use allstreams::*;
pub use removestream::*;
pub use tracked::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands for tracking Twitch and Mixer streams"]
#[commands(addstream, removestream, trackedstreams, allstreams)]
struct StreamTracking;
