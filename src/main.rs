mod core;

#[macro_use]
extern crate log;
#[macro_use]
extern crate failure;

use crate::core::logging;

use failure::Error;
use git_version::git_version;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GIT_VERSION: &str = git_version!();

#[tokio::main]
async fn main() -> Result<(), Error> {
    if let Err(e) = logging::initialize() {
        error!("{}", e);
        return Err(e);
    }
    Ok(())
}
