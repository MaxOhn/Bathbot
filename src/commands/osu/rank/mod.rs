use std::{borrow::Cow, sync::Arc};

use command_macros::{HasName, SlashCommand};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::GameModeOption,
    util::{interaction::InteractionCommand, InteractionCommandExt},
    BotResult, Context,
};

pub use self::{pp::*, score::*};

mod pp;
mod score;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "rank")]
/// How much is missing to reach the given rank?
pub enum Rank<'a> {
    #[command(name = "pp")]
    Pp(RankPp<'a>),
    #[command(name = "score")]
    Score(RankScore<'a>),
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "pp")]
/// How many pp are missing to reach the given rank?
pub struct RankPp<'a> {
    #[command(min_value = 1, max_value = 4_294_967_295)]
    /// Specify the target rank
    rank: u32,
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(min_value = 0.0)]
    /// Fill a top100 with scores of this many pp until the pp of the target rank are reached
    each: Option<f32>,
    /// Specify a country (code)
    country: Option<Cow<'a, str>>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "score")]
/// How much ranked score is missing to reach the given rank?
pub struct RankScore<'a> {
    #[command(min_value = 1)]
    /// Specify the target rank
    rank: usize,
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

async fn slash_rank(ctx: Arc<Context>, mut command: InteractionCommand) -> BotResult<()> {
    match Rank::from_interaction(command.input_data())? {
        Rank::Pp(args) => pp(ctx, (&mut command).into(), args).await,
        Rank::Score(args) => score(ctx, (&mut command).into(), args).await,
    }
}
