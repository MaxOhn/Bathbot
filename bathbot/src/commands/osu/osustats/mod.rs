use std::{borrow::Cow, sync::Arc};

use bathbot_macros::{HasMods, HasName, SlashCommand};
use bathbot_model::OsuStatsScoresOrder;
use eyre::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

pub use self::{counts::*, globals::*, list::*};
use crate::{
    commands::GameModeOption,
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

mod counts;
mod globals;
mod list;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "osustats",
    help = "Stats about scores that players have on maps' global leaderboards.\n\
        All data is provided by [osustats](https://osustats.ppy.sh/).\n\
        Note that the data usually __updates once per day__."
)]
/// Stats about player's appearances in maps' leaderboard
pub enum OsuStats<'a> {
    #[command(name = "count")]
    Count(OsuStatsCount<'a>),
    #[command(name = "players")]
    Players(OsuStatsPlayers<'a>),
    #[command(name = "scores")]
    Scores(OsuStatsScores<'a>),
}

#[derive(CommandModel, CreateCommand, Default, HasName)]
#[command(name = "count")]
/// Count how often a user appears on top of map leaderboards (same as `/osc`)
pub struct OsuStatsCount<'a> {
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

#[derive(CommandModel, CreateCommand)]
#[command(name = "players")]
/// All scores of a player that are on a map's global leaderboard
pub struct OsuStatsPlayers<'a> {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a country (code)
    country: Option<Cow<'a, str>>,
    #[command(min_value = 1, max_value = 100)]
    /// Specify a min rank between 1 and 100
    min_rank: Option<u32>,
    #[command(min_value = 1, max_value = 100)]
    /// Specify a max rank between 1 and 100
    max_rank: Option<u32>,
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(name = "scores")]
/// All scores of a player that are on a map's global leaderboard
pub struct OsuStatsScores<'a> {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
    /// Choose how the scores should be ordered
    sort: Option<OsuStatsScoresOrder>,
    #[command(help = "Filter out all scores that don't match the specified mods.\n\
    Mods must be given as `+mods` for included mods, `+mods!` for exact mods, \
    or `-mods!` for excluded mods.\n\
    Examples:\n\
    - `+hd`: Scores must have at least `HD` but can also have more other mods\n\
    - `+hdhr!`: Scores must have exactly `HDHR`\n\
    - `-ezhd!`: Scores must have neither `EZ` nor `HD` e.g. `HDDT` would get filtered out\n\
    - `-nm!`: Scores can not be nomod so there must be any other mod")]
    /// Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for
    /// excluded)
    mods: Option<Cow<'a, str>>,
    #[command(min_value = 1, max_value = 100)]
    /// Specify a min rank between 1 and 100
    min_rank: Option<u32>,
    #[command(min_value = 1, max_value = 100)]
    /// Specify a max rank between 1 and 100
    max_rank: Option<u32>,
    #[command(min_value = 0.0, max_value = 100.0)]
    /// Specify a min accuracy
    min_acc: Option<f32>,
    #[command(min_value = 0.0, max_value = 100.0)]
    /// Specify a max accuracy
    max_acc: Option<f32>,
    /// Reverse the resulting score list
    reverse: Option<bool>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

async fn slash_osustats(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    match OsuStats::from_interaction(command.input_data())? {
        OsuStats::Count(args) => count(ctx, (&mut command).into(), args).await,
        OsuStats::Players(args) => players(ctx, (&mut command).into(), args).await,
        OsuStats::Scores(args) => scores(ctx, (&mut command).into(), args).await,
    }
}
