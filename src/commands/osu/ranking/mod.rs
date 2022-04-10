use std::{borrow::Cow, sync::Arc};

use command_macros::SlashCommand;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{BotResult, Context};

pub use self::{countries::*, players::*};

mod countries;
mod players;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "ranking")]
/// Show the pp, ranked score, or country ranking
pub enum Ranking<'a> {
    #[command(name = "pp")]
    Pp(RankingPp<'a>),
    #[command(name = "score")]
    Score(RankingScore),
    #[command(name = "country")]
    Country(RankingCountry),
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "pp",
    help = "Display the global or country based performance points leaderboard"
)]
/// Show the pp ranking
pub struct RankingPp<'a> {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a country (code)
    country: Option<Cow<'a, str>>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "score", help = "Display the global ranked score leaderboard")]
/// Show the ranked score ranking
pub struct RankingScore {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "country",
    help = "Display the country leaderboard based on accumulated pp"
)]
/// Show the country ranking
pub struct RankingCountry {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
}

async fn slash_ranking(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    match Ranking::from_interaction(command.input_data())? {
        Ranking::Pp(args) => pp(ctx, command.into(), args).await,
        Ranking::Score(args) => score(ctx, command.into(), args).await,
        Ranking::Country(args) => country(ctx, command.into(), args).await,
    }
}
