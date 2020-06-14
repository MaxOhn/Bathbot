mod about;
mod authorities;
mod avatar;
mod command_count;
mod echo;
mod lyrics;
mod ping;
mod prune;
mod reload_verified;
mod role_assign;

pub use self::{
    about::*, authorities::*, avatar::*, command_count::*, echo::*, lyrics::*, ping::*, prune::*,
    reload_verified::*, role_assign::*,
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
    lyrics,
    reloadverified
)]
struct Utility;
