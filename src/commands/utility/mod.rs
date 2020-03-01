mod command_count;
mod help;
mod ping;
mod prune;
mod role_assign;
mod vc_role;

pub use self::{command_count::*, help::*, ping::*, prune::*, role_assign::*, vc_role::*};

use serenity::framework::standard::macros::group;

#[group]
#[commands(ping, commands, prune, roleassign, vcrole)]
struct Utility;
