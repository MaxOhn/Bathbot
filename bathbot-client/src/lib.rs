#[macro_use]
extern crate eyre;

#[macro_use]
extern crate tracing;

mod client;
mod error;
mod multipart;

pub use self::{client::Client, error::ClientError};
