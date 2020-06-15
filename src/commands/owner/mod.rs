mod add_bg;
mod reload_verified;

pub use self::{add_bg::*, reload_verified::*};

use serenity::framework::standard::macros::group;

#[group]
#[owners_only]
#[help_available(false)]
#[description = "Commands for the owners only"]
#[commands(reloadverified, addbg)]
struct Owner;
