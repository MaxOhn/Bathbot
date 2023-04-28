use std::{borrow::Cow, sync::Arc};

use bathbot_macros::{HasName, SlashCommand};
use eyre::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

pub use self::{pp::*, score::*};
use crate::{
    commands::GameModeOption,
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
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
        min_value = 1,
        max_value = 4_294_967_295,
        desc = "Specify the target rank"
    )]
    rank: u32,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        min_value = 0.0,
        desc = "Fill a top100 with scores of this many pp until the pp of the target rank are reached"
    )]
    each: Option<f32>,
    #[command(desc = "Specify a country (code)")]
    country: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "score",
    desc = "How much ranked score is missing to reach the given rank?"
)]
pub struct RankScore<'a> {
    #[command(min_value = 1, desc = "Specify the target rank")]
    rank: usize,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

async fn slash_rank(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    match Rank::from_interaction(command.input_data())? {
        Rank::Pp(args) => pp(ctx, (&mut command).into(), args).await,
        Rank::Score(args) => score(ctx, (&mut command).into(), args).await,
    }
}
