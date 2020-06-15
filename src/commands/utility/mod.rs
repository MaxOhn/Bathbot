mod about;
mod authorities;
mod avatar;
mod bg_tags;
mod command_count;
mod echo;
mod lyrics;
mod ping;
mod prune;
mod role_assign;

pub use self::{
    about::*, authorities::*, avatar::*, bg_tags::*, command_count::*, echo::*, lyrics::*, ping::*,
    prune::*, role_assign::*,
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
    bgtagsmanual,
    bgtags
)]
struct Utility;
