use std::borrow::Cow;

use bathbot_macros::{HasName, SlashCommand};
use bathbot_model::command_fields::GameModeOption;
use eyre::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{Id, marker::UserMarker};

pub use self::{pp::*, score::*};
use crate::{
    commands::{DISCORD_OPTION_DESC, DISCORD_OPTION_HELP},
    util::{InteractionCommandExt, interaction::InteractionCommand},
};

mod pp;
mod score;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "rank", desc = "How much is missing to reach the given rank?")]
pub enum Rank<'a> {
    #[command(name = "pp")]
    Pp(RankPp<'a>),
    #[command(name = "score")]
    Score(RankScore<'a>),
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "pp", desc = "How many pp are missing to reach the given rank?")]
pub struct RankPp<'a> {
    #[command(
        desc = "Specify the target rank",
        help = "Specify a target rank or target username.\n\
        Alternatively, prefix the value with a `+` so that it'll be interpreted as \"delta\" \
        meaning the current rank + the given value"
    )]
    rank: Cow<'a, str>,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        min_value = 0.0,
        desc = "Fill a top200 with scores of this many pp until the pp of the target rank are reached"
    )]
    each: Option<f32>,
    #[command(
        min_value = 1,
        max_value = 100,
        desc = "Specify an amount of scores to set to reach the target rank",
        help = "Specify an amount of scores to set to reach the target rank.\n\
        If `each` is set, this argument will be ignored"
    )]
    amount: Option<u8>,
    #[command(desc = "Specify a country (code)")]
    country: Option<Cow<'a, str>>,
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "score",
    desc = "How much ranked score is missing to reach the given rank?"
)]
pub struct RankScore<'a> {
    #[command(
        desc = "Specify the target rank",
        help = "Specify a target rank or target username.\n\
        Alternatively, prefix the value with a `+` so that it'll be interpreted as \"delta\" \
        meaning the current rank + the given value"
    )]
    rank: Cow<'a, str>,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
    discord: Option<Id<UserMarker>>,
}

#[derive(Copy, Clone)]
enum RankValue<'a> {
    Delta(u32),
    Raw(u32),
    Name(&'a str),
}

impl<'a> RankValue<'a> {
    fn parse(input: &'a str) -> Self {
        let Ok(rank) = input.parse() else {
            return Self::Name(input);
        };

        if input.starts_with('+') {
            Self::Delta(rank)
        } else {
            Self::Raw(rank)
        }
    }
}

async fn slash_rank(mut command: InteractionCommand) -> Result<()> {
    match Rank::from_interaction(command.input_data())? {
        Rank::Pp(args) => pp((&mut command).into(), args).await,
        Rank::Score(args) => score((&mut command).into(), args).await,
    }
}
