use bathbot_macros::{HasName, SlashCommand};
use eyre::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use crate::util::{interaction::InteractionCommand, InteractionCommandExt};

mod user;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "dailychallenge", desc = "Daily challenge statistics")]
pub enum DailyChallenge {
    #[command(name = "user")]
    User(DailyChallengeUser),
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "user", desc = "Daily challenge statistics of a user")]
pub struct DailyChallengeUser {
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

async fn slash_dailychallenge(mut command: InteractionCommand) -> Result<()> {
    match DailyChallenge::from_interaction(command.input_data())? {
        DailyChallenge::User(user) => user::user(command, user).await,
    }
}
