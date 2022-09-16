use std::{borrow::Cow, fmt, sync::Arc};

use command_macros::{HasMods, HasName, SlashCommand};
use eyre::Result;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

pub use self::{
    country_snipe_list::*, country_snipe_stats::*, player_snipe_list::*, player_snipe_stats::*,
    sniped::*, sniped_difference::*,
};

use super::prepare_score;

mod country_snipe_list;
mod country_snipe_stats;
mod player_snipe_list;
mod sniped_difference;

pub mod player_snipe_stats;
pub mod sniped;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "snipe",
    help = "National #1 related stats. \
    All data is provided by [huismetbenen](https://snipe.huismetbenen.nl).\n\
    Note that the data usually __updates once per week__."
)]
/// National #1 related data provided by huismetbenen
pub enum Snipe<'a> {
    #[command(name = "country")]
    Country(SnipeCountry<'a>),
    #[command(name = "player")]
    Player(SnipePlayer<'a>),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "country")]
/// Country related snipe stats
pub enum SnipeCountry<'a> {
    #[command(name = "list")]
    List(SnipeCountryList<'a>),
    #[command(name = "stats")]
    Stats(SnipeCountryStats<'a>),
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "list",
    help = "List all players of a country with a specific order based around #1 stats"
)]
/// Sort the country's #1 leaderboard
pub struct SnipeCountryList<'a> {
    /// Specify a country (code)
    country: Option<Cow<'a, str>>,
    #[command(help = "Specify the order of players.\n\
        Available orderings are `count` for amount of #1 scores, `pp` for average pp of #1 scores, \
        `stars` for average star rating of #1 scores, and `weighted_pp` for the total pp a user \
        would have if only their #1s would count towards it.")]
    /// Specify the order of players
    sort: Option<SnipeCountryListOrder>,
}

#[derive(Copy, Clone, CommandOption, CreateOption, Eq, PartialEq)]
pub enum SnipeCountryListOrder {
    #[option(name = "Count", value = "count")]
    Count,
    #[option(name = "PP", value = "pp")]
    Pp,
    #[option(name = "Stars", value = "stars")]
    Stars,
    #[option(name = "Weighted PP", value = "weighted_pp")]
    WeightedPp,
}

impl Default for SnipeCountryListOrder {
    fn default() -> Self {
        Self::Count
    }
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "stats")]
/// #1-count related stats for a country
pub struct SnipeCountryStats<'a> {
    /// Specify a country (code)
    country: Option<Cow<'a, str>>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "player")]
/// Player related snipe stats
pub enum SnipePlayer<'a> {
    #[command(name = "gain")]
    Gain(SnipePlayerGain<'a>),
    #[command(name = "list")]
    List(SnipePlayerList<'a>),
    #[command(name = "loss")]
    Loss(SnipePlayerLoss<'a>),
    #[command(name = "stats")]
    Stats(SnipePlayerStats<'a>),
    #[command(name = "sniped")]
    Sniped(SnipePlayerSniped<'a>),
}

#[derive(CommandModel, CreateCommand, Default, HasName)]
#[command(
    name = "gain",
    help = "Display all national #1 scores that a user acquired within the last week"
)]
/// Display a user's recent national #1 scores
pub struct SnipePlayerGain<'a> {
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

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(name = "list")]
/// List all national #1 scores of a player
pub struct SnipePlayerList<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
    )]
    /// Specify mods e.g. hdhr or nm
    mods: Option<Cow<'a, str>>,
    /// Specify the order of scores
    sort: Option<SnipePlayerListOrder>,
    /// Choose whether the list should be reversed
    reverse: Option<bool>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(Copy, Clone, CommandOption, CreateOption, Debug, Eq, PartialEq)]
pub enum SnipePlayerListOrder {
    #[option(name = "Accuracy", value = "acc")]
    Acc = 0,
    #[option(name = "Date", value = "date")]
    Date = 5,
    #[option(name = "Length", value = "len")]
    Length = 1,
    #[option(name = "Map approval date", value = "map_date")]
    MapDate = 2,
    #[option(name = "Misses", value = "misses")]
    Misses = 3,
    #[option(name = "PP", value = "pp")]
    Pp = 4,
    #[option(name = "Stars", value = "stars")]
    Stars = 6,
}

impl Default for SnipePlayerListOrder {
    fn default() -> Self {
        Self::Date
    }
}

impl fmt::Display for SnipePlayerListOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Acc => "accuracy",
            Self::Length => "length",
            Self::MapDate => "date_ranked",
            Self::Misses => "count_miss",
            Self::Pp => "pp",
            Self::Date => "date_set",
            Self::Stars => "sr",
        };

        f.write_str(name)
    }
}

#[derive(CommandModel, CreateCommand, Default, HasName)]
#[command(
    name = "loss",
    help = "Display all national #1 scores that a user lost within the last week"
)]
/// Display a user's recently lost national #1 scores
pub struct SnipePlayerLoss<'a> {
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

#[derive(CommandModel, CreateCommand, Default, HasName)]
#[command(name = "stats")]
/// Stats about a user's national #1 scores
pub struct SnipePlayerStats<'a> {
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

#[derive(CommandModel, CreateCommand, Default, HasName, SlashCommand)]
#[command(
    name = "sniped",
    help = "Display who sniped and was sniped the most by a user in last 8 weeks"
)]
/// Sniped users of the last 8 weeks
pub struct SnipePlayerSniped<'a> {
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

async fn slash_snipe(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    match Snipe::from_interaction(command.input_data())? {
        Snipe::Country(SnipeCountry::List(args)) => {
            country_list(ctx, (&mut command).into(), args).await
        }
        Snipe::Country(SnipeCountry::Stats(args)) => {
            country_stats(ctx, (&mut command).into(), args).await
        }
        Snipe::Player(SnipePlayer::Gain(args)) => {
            player_gain(ctx, (&mut command).into(), args).await
        }
        Snipe::Player(SnipePlayer::List(args)) => {
            player_list(ctx, (&mut command).into(), args).await
        }
        Snipe::Player(SnipePlayer::Loss(args)) => {
            player_loss(ctx, (&mut command).into(), args).await
        }
        Snipe::Player(SnipePlayer::Stats(args)) => {
            player_stats(ctx, (&mut command).into(), args).await
        }
        Snipe::Player(SnipePlayer::Sniped(args)) => {
            player_sniped(ctx, (&mut command).into(), args).await
        }
    }
}

async fn slash_snipeplayersniped(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = SnipePlayerSniped::from_interaction(command.input_data())?;

    player_sniped(ctx, (&mut command).into(), args).await
}
