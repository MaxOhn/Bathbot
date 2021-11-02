mod country_snipe_list;
mod country_snipe_stats;
mod player_snipe_list;
mod player_snipe_stats;
mod sniped;
mod sniped_difference;

pub use country_snipe_list::*;
pub use country_snipe_stats::*;
pub use player_snipe_list::*;
pub use player_snipe_stats::*;
pub use sniped::*;
pub use sniped_difference::*;

use std::sync::Arc;

use rosu_v2::prelude::Username;
use twilight_model::application::{
    command::CommandOptionChoice,
    interaction::{
        application_command::{CommandDataOption, CommandOptionValue},
        ApplicationCommand,
    },
};

use crate::{
    commands::{
        osu::{option_discord, option_mods, option_name},
        parse_discord, DoubleResultCow, MyCommand, MyCommandOption,
    },
    custom_client::SnipeScoreOrder,
    database::OsuData,
    util::{
        constants::common_literals::{
            ACC, ACCURACY, COUNTRY, DISCORD, MISSES, MODS, MODS_PARSE_FAIL, NAME, REVERSE, SORT,
        },
        matcher,
        osu::ModSelection,
        CountryCode, InteractionExt, MessageExt,
    },
    BotResult, Context, Error,
};

use super::{prepare_score, request_user, require_link};

enum SnipeCommandKind {
    CountryList(CountryListArgs),
    CountryStats(Option<CountryCode>),
    PlayerList(PlayerListArgs),
    PlayerStats(Option<Username>),
    Sniped(Option<Username>),
    SnipeGain(Option<Username>),
    SnipeLoss(Option<Username>),
}

impl SnipeCommandKind {
    async fn slash(ctx: &Context, command: &mut ApplicationCommand) -> DoubleResultCow<Self> {
        let option = command
            .data
            .options
            .pop()
            .ok_or(Error::InvalidCommandOptions)?;

        match option.value {
            CommandOptionValue::SubCommandGroup(options) => match option.name.as_str() {
                COUNTRY => Self::parse_country(ctx, options),
                "player" => Self::parse_player(ctx, command, options).await,
                _ => Err(Error::InvalidCommandOptions),
            },
            _ => Err(Error::InvalidCommandOptions),
        }
    }

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

    async fn parse_player_list(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut osu = ctx.psql().get_user_osu(command.user_id()?).await?;
        let mut order = None;
        let mut mods = None;
        let mut descending = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => osu = Some(value.into()),
                    SORT => match value.as_str() {
                        ACC => order = Some(SnipeScoreOrder::Accuracy),
                        "len" => order = Some(SnipeScoreOrder::Length),
                        "map_date" => order = Some(SnipeScoreOrder::MapApprovalDate),
                        MISSES => order = Some(SnipeScoreOrder::Misses),
                        "pp" => order = Some(SnipeScoreOrder::Pp),
                        "score_date" => order = Some(SnipeScoreOrder::ScoreDate),
                        "stars" => order = Some(SnipeScoreOrder::Stars),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    MODS => match matcher::get_mods(&value) {
                        Some(mods_) => mods = Some(mods_),
                        None => match value.parse() {
                            Ok(mods_) => mods = Some(ModSelection::Include(mods_)),
                            Err(_) => return Ok(Err(MODS_PARSE_FAIL.into())),
                        },
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Boolean(value) => {
                    let value = (option.name == REVERSE)
                        .then(|| value)
                        .ok_or(Error::InvalidCommandOptions)?;

                    descending = Some(!value);
                }
                CommandOptionValue::User(value) => match option.name.as_str() {
                    DISCORD => match parse_discord(ctx, value).await? {
                        Ok(osu_) => osu = Some(osu_),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        let args = PlayerListArgs {
            osu,
            order: order.unwrap_or_default(),
            mods,
            descending: descending.unwrap_or(true),
        };

        Ok(Ok(Self::PlayerList(args)))
    }
}

async fn parse_username(
    ctx: &Context,
    command: &ApplicationCommand,
    options: Vec<CommandDataOption>,
) -> DoubleResultCow<Option<Username>> {
    let mut username = None;

    for option in options {
        match option.value {
            CommandOptionValue::String(value) => match option.name.as_str() {
                NAME => username = Some(value.into()),
                _ => return Err(Error::InvalidCommandOptions),
            },
            CommandOptionValue::User(value) => match option.name.as_str() {
                DISCORD => match parse_discord(ctx,  value).await? {
                    Ok(osu) => username = Some(osu.into_username()),
                    Err(content) => return Ok(Err(content)),
                },
                _ => return Err(Error::InvalidCommandOptions),
            },
            _ => return Err(Error::InvalidCommandOptions),
        }
    }

    match username {
        Some(name) => Ok(Ok(Some(name))),
        None => ctx
            .psql()
            .get_user_osu(command.user_id()?)
            .await?
            .map(OsuData::into_username)
            .map(Ok)
            .transpose()
            .map(Ok),
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
                    "Failed to parse `{}` as country or country code.\n\
                    Be sure to specify a valid country or two ASCII letter country code.",
                    country
                );

                return Err(content);
            };

            if !country.snipe_supported(ctx) {
                let content = format!("The country acronym `{}` is not supported :(", country);

                return Err(content);
            }

            Ok(country)
        }
    }
}

pub async fn slash_snipe(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match SnipeCommandKind::slash(&ctx, &mut command).await? {
        Ok(SnipeCommandKind::CountryList(args)) => {
            _countrysnipelist(ctx, command.into(), args).await
        }
        Ok(SnipeCommandKind::CountryStats(country)) => {
            _countrysnipestats(ctx, command.into(), country).await
        }
        Ok(SnipeCommandKind::PlayerList(args)) => _playersnipelist(ctx, command.into(), args).await,
        Ok(SnipeCommandKind::PlayerStats(name)) => {
            _playersnipestats(ctx, command.into(), name).await
        }
        Ok(SnipeCommandKind::Sniped(name)) => _sniped(ctx, command.into(), name).await,
        Ok(SnipeCommandKind::SnipeGain(name)) => {
            _sniped_diff(ctx, command.into(), Difference::Gain, name).await
        }
        Ok(SnipeCommandKind::SnipeLoss(name)) => {
            _sniped_diff(ctx, command.into(), Difference::Loss, name).await
        }
        Err(content) => command.error(&ctx, content).await,
    }
}

fn option_country() -> MyCommandOption {
    MyCommandOption::builder(COUNTRY, "Specify a country (code)").string(Vec::new(), false)
}

fn subcommand_country() -> MyCommandOption {
    let country = option_country();

    let sort_choices = vec![
        CommandOptionChoice::String {
            name: "count".to_owned(),
            value: "count".to_owned(),
        },
        CommandOptionChoice::String {
            name: "pp".to_owned(),
            value: "pp".to_owned(),
        },
        CommandOptionChoice::String {
            name: "stars".to_owned(),
            value: "stars".to_owned(),
        },
        CommandOptionChoice::String {
            name: "weighted pp".to_owned(),
            value: "weighted_pp".to_owned(),
        },
    ];

    let sort_help = "Specify the order of players.\n\
        Available orderings are `count` for amount of #1 scores, `pp` for average pp of #1 scores, \
        `stars` for average star rating of #1 scores, and `weighted_pp` for the total pp a user \
        would have if only their #1s would count towards it.";

    let sort = MyCommandOption::builder(SORT, "Specify the order of players")
        .help(sort_help)
        .string(sort_choices, false);

    let list = MyCommandOption::builder("list", "Sort the country's #1 leaderboard")
        .help("List all players of a country with a specific order based around #1 stats")
        .subcommand(vec![country, sort]);

    let stats = MyCommandOption::builder("stats", "#1-count related stats for a country")
        .subcommand(vec![option_country()]);

    MyCommandOption::builder(COUNTRY, "Country related snipe stats")
        .subcommandgroup(vec![list, stats])
}

fn subcommand_player() -> MyCommandOption {
    let gain_description = "Display a user's recently acquired national #1 scores";

    let gain = MyCommandOption::builder("gain", gain_description)
        .help("Display all national #1 scores that a user acquired within the last week")
        .subcommand(vec![option_name(), option_discord()]);

    let name = option_name();
    let discord = option_discord();
    let mods = option_mods(false);

    let sort_choices = vec![
        CommandOptionChoice::String {
            name: ACCURACY.to_owned(),
            value: ACC.to_owned(),
        },
        CommandOptionChoice::String {
            name: "length".to_owned(),
            value: "len".to_owned(),
        },
        CommandOptionChoice::String {
            name: "map approval date".to_owned(),
            value: "map_date".to_owned(),
        },
        CommandOptionChoice::String {
            name: MISSES.to_owned(),
            value: MISSES.to_owned(),
        },
        CommandOptionChoice::String {
            name: "pp".to_owned(),
            value: "pp".to_owned(),
        },
        CommandOptionChoice::String {
            name: "score date".to_owned(),
            value: "score_date".to_owned(),
        },
        CommandOptionChoice::String {
            name: "stars".to_owned(),
            value: "stars".to_owned(),
        },
    ];

    let sort = MyCommandOption::builder(SORT, "Specify the order of scores")
        .help("Specify the order of scores. Defaults to `pp`.")
        .string(sort_choices, false);

    let reverse = MyCommandOption::builder(REVERSE, "Choose whether the list should be reversed")
        .boolean(false);

    let list = MyCommandOption::builder("list", "List all national #1 scores of a player")
        .subcommand(vec![name, mods, sort, reverse, discord]);

    let loss_description = "Display a user's recently lost national #1 scores";

    let loss = MyCommandOption::builder("loss", loss_description)
        .help("Display all national #1 scores that a user lost within the last week")
        .subcommand(vec![option_name(), option_discord()]);

    let stats = MyCommandOption::builder("stats", "Stats about a user's national #1 scores")
        .subcommand(vec![option_name(), option_discord()]);

    let targets_help = "Display who sniped and was sniped the most by a user in last 8 weeks";

    let targets = MyCommandOption::builder("targets", "Sniped users of the last 8 weeks")
        .help(targets_help)
        .subcommand(vec![option_name(), option_discord()]);

    MyCommandOption::builder("player", "Player related snipe stats")
        .subcommandgroup(vec![gain, list, loss, stats, targets])
}

pub fn define_snipe() -> MyCommand {
    let help = "National #1 related stats. \
        All data is provided by [huismetbenen](https://snipe.huismetbenen.nl).\n\
        Note that the data usually __updates once per week__.";

    MyCommand::new("snipe", "National #1 related data provided by huismetbenen")
        .help(help)
        .options(vec![subcommand_country(), subcommand_player()])
}
