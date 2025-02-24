use std::borrow::Cow;

use bathbot_macros::SlashCommand;
use bathbot_model::command_fields::GameModeOption;
use eyre::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};

pub use self::{countries::*, players::*};
use crate::util::{InteractionCommandExt, interaction::InteractionCommand};

mod countries;
mod players;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "ranking",
    desc = "Show the pp, ranked score, or country ranking"
)]
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
    desc = "Show the pp ranking",
    help = "Display the global or country based performance points leaderboard"
)]
pub struct RankingPp<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a country (code)")]
    country: Option<Cow<'a, str>>,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "score",
    desc = "Show the ranked score ranking",
    help = "Display the global ranked score leaderboard"
)]
pub struct RankingScore {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
}

impl From<Option<GameModeOption>> for RankingScore {
    fn from(mode: Option<GameModeOption>) -> Self {
        Self { mode }
    }
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "country",
    desc = "Show the country ranking",
    help = "Display the country leaderboard based on accumulated pp"
)]
pub struct RankingCountry {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
}

impl From<Option<GameModeOption>> for RankingCountry {
    fn from(mode: Option<GameModeOption>) -> Self {
        Self { mode }
    }
}

async fn slash_ranking(mut command: InteractionCommand) -> Result<()> {
    match Ranking::from_interaction(command.input_data())? {
        Ranking::Pp(args) => pp((&mut command).into(), args).await,
        Ranking::Score(args) => score((&mut command).into(), args).await,
        Ranking::Country(args) => country((&mut command).into(), args).await,
    }
}
