#[macro_use]
extern crate tracing;

mod middleware;
mod routes;
mod server;
mod standby;
mod state;

pub use self::{
    server::Server,
    standby::{AuthenticationStandby, AuthenticationStandbyError},
    state::AppStateBuilder,
};
