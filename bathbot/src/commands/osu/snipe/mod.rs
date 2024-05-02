use std::borrow::Cow;

use bathbot_macros::{HasMods, HasName, SlashCommand};
use bathbot_model::{SnipeCountryListOrder, SnipePlayerListOrder};
use eyre::Result;
use rosu_v2::model::GameMode;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

pub use self::{
    country_snipe_list::*, country_snipe_stats::*, player_snipe_list::*, player_snipe_stats::*,
    sniped::*, sniped_difference::*,
};
use crate::util::{interaction::InteractionCommand, InteractionCommandExt};

mod country_snipe_list;
mod country_snipe_stats;
mod player_snipe_list;
mod sniped_difference;

pub mod player_snipe_stats;
pub mod sniped;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "snipe",
    desc = "National #1 related data",
    help = "National #1 related stats. Data is provided by:\n\
    - osu!standard: [huismetbenen](https://snipe.huismetbenen.nl)\n\
    - osu!mania: [kittenroleplay](https://snipes.kittenroleplay.com)\n\
    Note that the data usually __updates once per week__."
)]
pub enum Snipe<'a> {
    #[command(name = "country")]
    Country(SnipeCountry<'a>),
    #[command(name = "player")]
    Player(SnipePlayer<'a>),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "country", desc = "Country related snipe stats")]
pub enum SnipeCountry<'a> {
    #[command(name = "list")]
    List(SnipeCountryList<'a>),
    #[command(name = "stats")]
    Stats(SnipeCountryStats<'a>),
}

#[derive(Copy, Clone, CommandOption, CreateOption, Default)]
pub enum SnipeGameMode {
    #[default]
    #[option(name = "osu", value = "osu")]
    Osu,
    #[option(name = "mania", value = "mania")]
    Mania,
}

impl From<SnipeGameMode> for GameMode {
    fn from(mode: SnipeGameMode) -> Self {
        match mode {
            SnipeGameMode::Osu => Self::Osu,
            SnipeGameMode::Mania => Self::Mania,
        }
    }
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "list",
    desc = "Sort the country's #1 leaderboard",
    help = "List all players of a country with a specific order based around #1 stats"
)]
pub struct SnipeCountryList<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<SnipeGameMode>,
    #[command(desc = "Specify a country (code)")]
    country: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify the order of players",
        help = "Specify the order of players.\n\
        Available orderings are `count` for amount of #1 scores, `pp` for average pp of #1 scores, \
        `stars` for average star rating of #1 scores, and `weighted_pp` for the total pp a user \
        would have if only their #1s would count towards it."
    )]
    sort: Option<SnipeCountryListOrder>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "stats", desc = "#1-count related stats for a country")]
pub struct SnipeCountryStats<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<SnipeGameMode>,
    #[command(desc = "Specify a country (code)")]
    country: Option<Cow<'a, str>>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "player", desc = "Player related snipe stats")]
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
    desc = "Display a user's recent national #1 scores",
    help = "Display all national #1 scores that a user acquired within the last week"
)]
pub struct SnipePlayerGain<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<SnipeGameMode>,
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

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(name = "list", desc = "List all national #1 scores of a player")]
pub struct SnipePlayerList<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<SnipeGameMode>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify mods e.g. hdhr or nm",
        help = "Specify mods either directly or through the explicit \
        `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
    )]
    mods: Option<Cow<'a, str>>,
    #[command(desc = "Specify the order of scores")]
    sort: Option<SnipePlayerListOrder>,
    #[command(desc = "Choose whether the list should be reversed")]
    reverse: Option<bool>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, Default, HasName)]
#[command(
    name = "loss",
    desc = "Display a user's recently lost national #1 scores",
    help = "Display all national #1 scores that a user lost within the last week"
)]
pub struct SnipePlayerLoss<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<SnipeGameMode>,
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

#[derive(CommandModel, CreateCommand, Default, HasName)]
#[command(name = "stats", desc = "Stats about a user's national #1 scores")]
pub struct SnipePlayerStats<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<SnipeGameMode>,
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

#[derive(CommandModel, CreateCommand, Default, HasName, SlashCommand)]
#[command(
    name = "sniped",
    desc = "Sniped users of the last 8 weeks",
    help = "Display who sniped and was sniped the most by a user in last 8 weeks"
)]
pub struct SnipePlayerSniped<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<SnipeGameMode>,
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

async fn slash_snipe(mut command: InteractionCommand) -> Result<()> {
    match Snipe::from_interaction(command.input_data())? {
        Snipe::Country(SnipeCountry::List(args)) => country_list((&mut command).into(), args).await,
        Snipe::Country(SnipeCountry::Stats(args)) => {
            country_stats((&mut command).into(), args).await
        }
        Snipe::Player(SnipePlayer::Gain(args)) => player_gain((&mut command).into(), args).await,
        Snipe::Player(SnipePlayer::List(args)) => player_list((&mut command).into(), args).await,
        Snipe::Player(SnipePlayer::Loss(args)) => player_loss((&mut command).into(), args).await,
        Snipe::Player(SnipePlayer::Stats(args)) => player_stats((&mut command).into(), args).await,
        Snipe::Player(SnipePlayer::Sniped(args)) => {
            player_sniped((&mut command).into(), args).await
        }
    }
}

async fn slash_snipeplayersniped(mut command: InteractionCommand) -> Result<()> {
    let args = SnipePlayerSniped::from_interaction(command.input_data())?;

    player_sniped((&mut command).into(), args).await
}
