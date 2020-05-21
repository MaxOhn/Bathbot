mod about;
mod authorities;
mod avatar;
mod command_count;
mod echo;
mod help;
mod lyrics;
mod ping;
mod prune;
mod role_assign;
mod vc_role;

pub use self::{
    about::*, authorities::*, avatar::*, command_count::*, echo::*, help::*, lyrics::*, ping::*,
    prune::*, role_assign::*, vc_role::*,
};

use serenity::framework::standard::macros::group;

#[group]
#[description = "Various utility commands"]
#[commands(
    ping,
    commands,
    about,
    avatar,
    echo,
    prune,
    authorities,
    roleassign,
    vcrole,
    lyrics
)]
struct Utility;
