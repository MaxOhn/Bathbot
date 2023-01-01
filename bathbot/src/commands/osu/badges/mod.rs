use std::{cmp::Reverse, sync::Arc};

use bathbot_macros::{HasName, SlashCommand};
use eyre::Result;
use twilight_interactions::command::{
    AutocompleteValue, CommandModel, CommandOption, CreateCommand, CreateOption,
};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    core::Context,
    custom_client::OsekaiBadge,
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

use self::{query::*, user::*};

mod query;
mod user;

#[derive(CreateCommand, SlashCommand)]
#[command(name = "badges")]
#[allow(dead_code)]
/// Display info about badges
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
#[command(name = "query")]
#[allow(dead_code)]
/// Display all badges matching the query
pub struct BadgesQuery {
    #[command(autocomplete = true)]
    /// Specify the badge name or acronym
    name: String,
    /// Choose how the badges should be ordered
    sort: Option<BadgesOrder>,
}

#[derive(CommandModel)]
#[command(autocomplete = true)]
struct BadgesQuery_ {
    name: AutocompleteValue<String>,
    sort: Option<BadgesOrder>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "user")]
/// Display all badges of a user
pub struct BadgesUser {
    /// Specify a username
    name: Option<String>,
    /// Choose how the badges should be ordered
    sort: Option<BadgesOrder>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
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
