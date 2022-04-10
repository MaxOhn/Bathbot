
use std::{borrow::Cow, sync::Arc};

use command_macros::{SlashCommand, HasName};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::application::{
    interaction::{
        ApplicationCommand,
    },
};

use crate::{
    custom_client::SnipeScoreOrder,
    util::{
        matcher,
        osu::ModSelection,
        CountryCode,
    },
    BotResult, Context, Error,
};

pub use self::{
    country_snipe_list::*, country_snipe_stats::*, player_snipe_list::*, player_snipe_stats::*,
    sniped::*, sniped_difference::*,
};

use super::{prepare_score, require_link};

mod country_snipe_list;
mod country_snipe_stats;
mod player_snipe_list;
mod sniped_difference;

pub mod player_snipe_stats;
pub mod sniped;

impl SnipeCommandKind {
    fn parse_country(ctx: &Context, mut options: Vec<CommandDataOption>) -> DoubleResultCow<Self> {
        let option = options.pop().ok_or(Error::InvalidCommandOptions)?;

        match option.value {
            CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                "list" => Self::parse_country_list(ctx, options),
                "stats" => Self::parse_country_stats(ctx, options),
                _ => Err(Error::InvalidCommandOptions),
            },
            _ => Err(Error::InvalidCommandOptions),
        }
    }

    async fn parse_player(
        ctx: &Context,
        command: &ApplicationCommand,
        mut options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let option = options.pop().ok_or(Error::InvalidCommandOptions)?;

        match option.value {
            CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                "gain" => {
                    let name = match parse_username(ctx, command, options).await? {
                        Ok(name) => name,
                        Err(content) => return Ok(Err(content)),
                    };

                    Ok(Ok(Self::SnipeGain(name)))
                }
                "list" => Self::parse_player_list(ctx, command, options).await,
                "loss" => {
                    let name = match parse_username(ctx, command, options).await? {
                        Ok(name) => name,
                        Err(content) => return Ok(Err(content)),
                    };

                    Ok(Ok(Self::SnipeLoss(name)))
                }
                "stats" => {
                    let name = match parse_username(ctx, command, options).await? {
                        Ok(name) => name,
                        Err(content) => return Ok(Err(content)),
                    };

                    Ok(Ok(Self::PlayerStats(name)))
                }
                "targets" => {
                    let name = match parse_username(ctx, command, options).await? {
                        Ok(name) => name,
                        Err(content) => return Ok(Err(content)),
                    };

                    Ok(Ok(Self::Sniped(name)))
                }
                _ => Err(Error::InvalidCommandOptions),
            },
            _ => Err(Error::InvalidCommandOptions),
        }
    }

    fn parse_country_list(ctx: &Context, options: Vec<CommandDataOption>) -> DoubleResultCow<Self> {
        let mut country = None;
        let mut sort = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    COUNTRY => match parse_country_code(ctx, value) {
                        Ok(country_) => country = Some(country_),
                        Err(content) => return Ok(Err(content.into())),
                    },
                    SORT => match value.as_str() {
                        "count" => sort = Some(SnipeOrder::Count),
                        "pp" => sort = Some(SnipeOrder::Pp),
                        "stars" => sort = Some(SnipeOrder::Stars),
                        "weighted_pp" => sort = Some(SnipeOrder::WeightedPp),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        let sort = sort.unwrap_or_default();

        Ok(Ok(Self::CountryList(CountryListArgs { country, sort })))
    }

    fn parse_country_stats(
        ctx: &Context,
        mut options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut country = None;

        if let Some(option) = options.pop() {
            match option.value {
                CommandOptionValue::String(value) => {
                    let value = (option.name == COUNTRY)
                        .then(|| parse_country_code(ctx, value))
                        .ok_or(Error::InvalidCommandOptions)?;

                    match value {
                        Ok(country_) => country = Some(country_),
                        Err(content) => return Ok(Err(content.into())),
                    }
                }
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        Ok(Ok(Self::CountryStats(country)))
    }
}

fn parse_country_code(ctx: &Context, mut country: String) -> Result<CountryCode, String> {
    match country.as_str() {
        "global" | "world" => Ok("global".into()),
        _ => {
            let country = if country.len() == 2 && country.is_ascii() {
                country.make_ascii_uppercase();

                country.into()
            } else if let Some(code) = CountryCode::from_name(&country) {
                code
            } else {
                let content = format!(
                    "Failed to parse `{country}` as country or country code.\n\
                    Be sure to specify a valid country or two ASCII letter country code."
                );

                return Err(content);
            };

            if !country.snipe_supported(ctx) {
                let content = format!("The country acronym `{country}` is not supported :(");

                return Err(content);
            }

            Ok(country)
        }
    }
}


// TODO: vim formating goes bruh mode

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "snipe", help= "National #1 related stats. \
        All data is provided by [huismetbenen](https://snipe.huismetbenen.nl).\n\
        Note that the data usually __updates once per week__.")]
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
    List(SnipeCountryList<'a>),
    Stats(SnipeCountryStats<'a>),
}

#[derive(CommandModel, CreateCommand)]
#[comand(name = "list", help = "List all players of a country with a specific order based around #1 stats")]
/// Sort the country's #1 leaderboard
pub struct SnipeCountryList<'a> {
    /// Specify a country (code)
    country: Option<Cow<'a, str>>
        #[command(help = "Specify the order of players.\n\
        Available orderings are `count` for amount of #1 scores, `pp` for average pp of #1 scores, \
        `stars` for average star rating of #1 scores, and `weighted_pp` for the total pp a user \
        would have if only their #1s would count towards it.")]
        /// Specify the order of players
        sort: Option<SnipeCountryListOrder>,
}

#[derive(CommandOption, CreateOptio)]
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

#[derive(CommandModel, CreateCommand)]
#[comand(name = "stats")]
/// #1-count related stats for a country
pub struct SnipeCountryStats<'a> {
    /// Specify a country (code)
    country: Option<Cow<'a, str>>
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "player")]
/// Player related snipe stats
pub enum SnipePlayer<'a> {
    Gain(SnipePlayerGain<'a>),
    List(SnipePlayerList<'a>),
    Loss(SnipePlayerLoss<'a>),
    Stats(SnipePlayerStats<'a>),
    Sniped(SnipePlayerSniped<'a>),
}

#[derive(CommandModel, CreateCommand, HasName)]
#[comand(name = "gain", help = "Display all national #1 scores that a user acquired within the last week")]
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

#[derive(CommandModel, CreateCommand, HasName)]
#[comand(name = "list")]
/// List all national #1 scores of a player
pub struct SnipePlayerList<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
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

#[derive(CommandOption, CreateOption)]
pub enum SnipePlayerListOrder {
    #[option(name = "Accuracy", value = "acc")]
    Acc,
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "Length", value = "len")]
    Length,
    #[option(name = "Map approval date", value = "map_date")]
    MapDate,
    #[option(name = "Misses", value = "misses")]
    Misses,
    #[option(name = "PP", value = "pp")]
    Pp,
    #[option(name = "Stars", value = "stars")]
    Stars,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[comand(name = "loss", help = "Display all national #1 scores that a user lost within the last week")]
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

#[derive(CommandModel, CreateCommand, HasName)]
#[comand(name = "stats")]
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

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[comand(name = "sniped", help = "Display who sniped and was sniped the most by a user in last 8 weeks")]
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

async fn slash_snipe(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    match Snipe::from_interaction(command.input_data())? {
        Snipe::Country(SnipeCountry::List(args)) => country_list(ctx, command.into(), args).await,
        Snipe::Country(SnipeCountry::Stats(args)) => country_stats(ctx, command.into(), args).await,
        Snipe::Player(SnipePlayer::Gain(args)) => player_gain(ctx, command.into(), args).await,
        Snipe::Player(SnipePlayer::List(args)) => player_list(ctx, command.into(), args).await,
        Snipe::Player(SnipePlayer::Loss(args)) => player_loss(ctx, command.into(), args).await,
        Snipe::Player(SnipePlayer::Stats(args)) => player_stats(ctx, command.into(), args).await,
        Snipe::Player(SnipePlayer::Sniped(args)) => player_sniped(ctx, command.into(), args).await,
    }
}

async fn slash_sniped(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    let args = SnipePlayerSniped::from_interaction(command.input_data())?;

    player_sniped(ctx, command.into(), args).await
}
