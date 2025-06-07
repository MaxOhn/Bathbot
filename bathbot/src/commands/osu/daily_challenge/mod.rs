use std::borrow::Cow;

use bathbot_macros::{HasName, SlashCommand};
use eyre::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{Id, marker::UserMarker};

pub use self::today::DailyChallengeDay;
use crate::{
    commands::{DISCORD_OPTION_DESC, DISCORD_OPTION_HELP},
    util::{InteractionCommandExt, interaction::InteractionCommand},
};

mod today;
mod user;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "dailychallenge", desc = "Daily challenge statistics")]
pub enum DailyChallenge<'a> {
    #[command(name = "user")]
    User(DailyChallengeUser<'a>),
    #[command(name = "today")]
    Today(DailyChallengeToday),
}

const DC_USER_DESC: &str = "Daily challenge statistics of a user";

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "user", desc = DC_USER_DESC)]
pub struct DailyChallengeUser<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
    discord: Option<Id<UserMarker>>,
}

const DC_TODAY_DESC: &str = "Check the map and leaderboard of today's daily challenge";

#[derive(CommandModel, CreateCommand)]
#[command(name = "today", desc = DC_TODAY_DESC)]
pub struct DailyChallengeToday;

async fn slash_dailychallenge(mut command: InteractionCommand) -> Result<()> {
    match DailyChallenge::from_interaction(command.input_data())? {
        DailyChallenge::User(user) => user::user((&mut command).into(), user).await,
        DailyChallenge::Today(_) => today::today((&mut command).into()).await,
    }
}
