mod authorities;
mod command_count;
mod help;
mod lyrics;
mod ping;
mod prune;
mod role_assign;
mod vc_role;

pub use self::{
    authorities::*, command_count::*, help::*, lyrics::*, ping::*, prune::*, role_assign::*,
    vc_role::*,
};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Various utility commands"]
#[commands(ping, commands, prune, authorities, roleassign, vcrole, lyrics)]
struct Utility;
