use std::{cmp::Reverse, sync::Arc};

use bathbot_macros::{HasName, SlashCommand};
use bathbot_model::OsekaiBadge;
use eyre::Result;
use twilight_interactions::command::{
    AutocompleteValue, CommandModel, CommandOption, CreateCommand, CreateOption,
};
use twilight_model::id::{marker::UserMarker, Id};

use self::{query::*, user::*};
use crate::{
    core::Context,
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

mod query;
mod user;

#[derive(CreateCommand, SlashCommand)]
#[command(name = "badges", desc = "Display info about badges")]
#[allow(dead_code)]
pub enum Badges {
    #[command(name = "query")]
    Query(BadgesQuery),
    #[command(name = "user")]
    User(BadgesUser),
}

#[derive(CommandModel)]
enum Badges_ {
    #[command(name = "query")]
    Query(BadgesQuery_),
    #[command(name = "user")]
    User(BadgesUser),
}

#[derive(CreateCommand)]
#[command(name = "query", desc = "Display all badges matching the query")]
#[allow(dead_code)]
pub struct BadgesQuery {
    #[command(autocomplete = true, desc = "Specify the badge name or acronym")]
    name: String,
    #[command(desc = "Choose how the badges should be ordered")]
    sort: Option<BadgesOrder>,
}

#[derive(CommandModel)]
#[command(autocomplete = true)]
struct BadgesQuery_ {
    name: AutocompleteValue<String>,
    sort: Option<BadgesOrder>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "user", desc = "Display all badges of a user")]
pub struct BadgesUser {
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(desc = "Choose how the badges should be ordered")]
    sort: Option<BadgesOrder>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandOption, CreateOption)]
pub enum BadgesOrder {
    #[option(name = "Alphabetically", value = "alphabet")]
    Alphabet,
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "Owner count", value = "owners")]
    Owners,
}

impl BadgesOrder {
    fn apply(self, badges: &mut [OsekaiBadge]) {
        match self {
            Self::Alphabet => badges.sort_unstable_by(|a, b| a.name.cmp(&b.name)),
            Self::Date => badges.sort_unstable_by_key(|badge| Reverse(badge.awarded_at)),
            Self::Owners => badges.sort_unstable_by_key(|badge| Reverse(badge.users.len())),
        }
    }
}

impl Default for BadgesOrder {
    fn default() -> Self {
        Self::Date
    }
}

pub async fn slash_badges(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    match Badges_::from_interaction(command.input_data())? {
        Badges_::Query(args) => query(ctx, command, args).await,
        Badges_::User(args) => user(ctx, (&mut command).into(), args).await,
    }
}
