use std::borrow::Cow;

use bathbot_macros::{HasName, SlashCommand};
use eyre::Result;
use profile::relax_profile;
use top::relax_top;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use crate::util::{interaction::InteractionCommand, InteractionCommandExt};

pub mod profile;
pub mod top;
#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "relax",
    desc = "Relax leaderboards related data",
    help = "Relax leaderboards data, provided by [Relaxation Vault](https://rx.stanr.info/)"
)]
pub enum Relax<'a> {
    #[command(name = "profile")]
    Profile(RelaxProfile<'a>),
    #[command(name = "top")]
    Top(RelaxTop<'a>),
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "profile",
    desc = "Show user's relax profile",
    help = "Show user's relax profile, as provided by [Relaxation Vault](https://rx.stanr.info/)"
)]
pub struct RelaxProfile<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "top",
    desc = "Show user's relax top plays",
    help = "Show user's relax top plays, as provided by [Relaxation Vault](https://rx.stanr.info/)"
)]
pub struct RelaxTop<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

pub async fn slash_relax(mut command: InteractionCommand) -> Result<()> {
    match Relax::from_interaction(command.input_data())? {
        Relax::Profile(args) => relax_profile((&mut command).into(), args).await,

        Relax::Top(args) => relax_top((&mut command).into(), args).await,
    }
}
