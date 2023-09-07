#[macro_use]
extern crate eyre;

#[macro_use]
extern crate tracing;

pub use self::database::Database;

mod database;
mod impls;
mod util;

pub mod model;
