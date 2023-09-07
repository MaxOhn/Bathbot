pub use self::{
    flags::CommandFlags,
    origin::{CommandOrigin, OwnedCommandOrigin},
};

mod flags;
mod origin;

pub mod checks;
pub mod interaction;
pub mod prefix;
