use std::{borrow::Cow, sync::Arc};

use bathbot_macros::SlashCommand;
use eyre::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};

use crate::{
    commands::GameModeOption,
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

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

impl From<Option<GameModeOption>> for RankingScore {
    fn from(mode: Option<GameModeOption>) -> Self {
        Self { mode }
    }
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

impl From<Option<GameModeOption>> for RankingCountry {
    fn from(mode: Option<GameModeOption>) -> Self {
        Self { mode }
    }
}

async fn slash_ranking(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    match Ranking::from_interaction(command.input_data())? {
        Ranking::Pp(args) => pp(ctx, (&mut command).into(), args).await,
        Ranking::Score(args) => score(ctx, (&mut command).into(), args).await,
        Ranking::Country(args) => country(ctx, (&mut command).into(), args).await,
    }
}
