use std::{cmp::Reverse, sync::Arc};

use command_macros::{HasName, SlashCommand};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::{
    application::interaction::ApplicationCommand,
    id::{marker::UserMarker, Id},
};

use crate::{core::Context, custom_client::OsekaiBadge, util::ApplicationCommandExt, BotResult};

pub use query::handle_autocomplete as handle_badge_autocomplete;

use self::{query::*, user::*};

mod query;
mod user;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "badges")]
/// Display info about badges
pub enum Badges {
    #[command(name = "query")]
    Query(BadgesQuery),
    #[command(name = "user")]
    User(BadgesUser),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "query")]
/// Display all badges matching the query
pub struct BadgesQuery {
    #[command(autocomplete = true)]
    /// Specify the badge name or acronym
    name: String,
    /// Choose how the badges should be ordered
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

async fn slash_badges(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    match Badges::from_interaction(command.input_data())? {
        Badges::Query(args) => query(ctx, command, args).await,
        Badges::User(args) => user(ctx, command.into(), args).await,
    }
}
