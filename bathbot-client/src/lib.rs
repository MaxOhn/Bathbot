#[macro_use]
extern crate eyre;

#[macro_use]
extern crate tracing;

mod client;
mod discord;
mod error;
mod github;
mod metrics;
mod miss_analyzer;
mod multipart;
mod osekai;
mod osu;
mod osustats;
mod osutrack;
mod relax;
mod respektive;
mod site;
mod snipe;
mod twitch;

use self::site::Site;
pub use self::{client::Client, error::ClientError};

static MY_USER_AGENT: &str = env!("CARGO_PKG_NAME");
