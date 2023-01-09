#[macro_use]
extern crate eyre;

#[macro_use]
extern crate tracing;

mod client;
mod discord;
mod error;
mod metrics;
mod multipart;
mod osu;
mod site;
mod twitch;

pub use self::{client::Client, error::ClientError};

use self::site::Site;

static MY_USER_AGENT: &str = env!("CARGO_PKG_NAME");
