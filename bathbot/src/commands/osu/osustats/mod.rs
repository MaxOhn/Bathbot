use std::borrow::Cow;

use bathbot_macros::{HasMods, HasName, SlashCommand};
use bathbot_model::{OsuStatsBestTimeframe, OsuStatsScoresOrder, command_fields::GameModeOption};
use eyre::Result;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{Id, marker::UserMarker};

use self::best::*;
pub use self::{counts::*, globals::*, list::*};
use crate::util::{InteractionCommandExt, interaction::InteractionCommand};

mod best;
mod counts;
mod globals;
mod list;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "osustats",
    desc = "Stats about player's appearances in maps' leaderboard",
    help = "Stats about scores that players have on maps' global leaderboards.\n\
    All data is provided by [osustats](https://osustats.ppy.sh/).\n\
    Note that the data usually __updates once per day__."
)]
pub enum OsuStats<'a> {
    #[command(name = "count")]
    Count(OsuStatsCount<'a>),
    #[command(name = "players")]
    Players(OsuStatsPlayers<'a>),
    #[command(name = "scores")]
    Scores(OsuStatsScores<'a>),
    #[command(name = "best")]
    Best(OsuStatsBest),
}

#[derive(CommandModel, CreateCommand, Default, HasName)]
#[command(
    name = "count",
    desc = "Count how often a user appears on top of map leaderboards (same as `/osc`)"
)]
pub struct OsuStatsCount<'a> {
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

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "players",
    desc = "All scores of a player that are on a map's global leaderboard"
)]
pub struct OsuStatsPlayers<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a country (code)")]
    country: Option<Cow<'a, str>>,
    #[command(
        min_value = 1,
        max_value = 100,
        desc = "Specify a min rank between 1 and 100"
    )]
    min_rank: Option<u32>,
    #[command(
        min_value = 1,
        max_value = 100,
        desc = "Specify a max rank between 1 and 100"
    )]
    max_rank: Option<u32>,
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(
    name = "scores",
    desc = "All scores of a player that are on a map's global leaderboard"
)]
pub struct OsuStatsScores<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(desc = "Choose how the scores should be ordered")]
    sort: Option<OsuStatsScoresOrder>,
    #[command(
        desc = "Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)",
        help = "Filter out all scores that don't match the specified mods.\n\
        Mods must be given as `+mods` for included mods, `+mods!` for exact mods, \
        or `-mods!` for excluded mods.\n\
        Examples:\n\
        - `+hd`: Scores must have at least `HD` but can also have more other mods\n\
        - `+hdhr!`: Scores must have exactly `HDHR`\n\
        - `-ezhd!`: Scores must have neither `EZ` nor `HD` e.g. `HDDT` would get filtered out\n\
        - `-nm!`: Scores can not be nomod so there must be any other mod"
    )]
    mods: Option<Cow<'a, str>>,
    #[command(
        min_value = 1,
        max_value = 100,
        desc = "Specify a min rank between 1 and 100"
    )]
    min_rank: Option<u32>,
    #[command(
        min_value = 1,
        max_value = 100,
        desc = "Specify a max rank between 1 and 100"
    )]
    max_rank: Option<u32>,
    #[command(min_value = 0.0, max_value = 100.0, desc = "Specify a min accuracy")]
    min_acc: Option<f32>,
    #[command(min_value = 0.0, max_value = 100.0, desc = "Specify a max accuracy")]
    max_acc: Option<f32>,
    #[command(desc = "Reverse the resulting score list")]
    reverse: Option<bool>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "best", desc = "Global top scores of a certain timeframe")]
pub struct OsuStatsBest {
    #[command(desc = "Only show scores of this timeframe")]
    timeframe: OsuStatsBestTimeframe,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Choose how the scores should be ordered")]
    sort: Option<OsuStatsBestSort>,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum OsuStatsBestSort {
    #[option(name = "Accuracy", value = "acc")]
    Accuracy,
    #[option(name = "Combo", value = "combo")]
    Combo,
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "Leaderboard Position", value = "pos")]
    LeaderboardPosition,
    #[option(name = "Misses", value = "miss")]
    Misses,
    #[option(name = "PP", value = "pp")]
    Pp,
    #[option(name = "Score", value = "score")]
    Score,
}

impl Default for OsuStatsBestSort {
    fn default() -> Self {
        Self::Pp
    }
}

async fn slash_osustats(mut command: InteractionCommand) -> Result<()> {
    match OsuStats::from_interaction(command.input_data())? {
        OsuStats::Count(args) => count((&mut command).into(), args).await,
        OsuStats::Players(args) => players((&mut command).into(), args).await,
        OsuStats::Scores(args) => scores((&mut command).into(), args).await,
        OsuStats::Best(args) => recentbest((&mut command).into(), args).await,
    }
}
