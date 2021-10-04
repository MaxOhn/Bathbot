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

use super::{prepare_score, request_user, require_link};

use crate::{
    commands::{
        osu::{option_discord, option_mods, option_name},
        MyCommand, MyCommandOption,
    },
    custom_client::SnipeScoreOrder,
    util::{
        constants::common_literals::{
            ACC, ACCURACY, COUNTRY, DISCORD, MISSES, MODS, MODS_PARSE_FAIL, NAME, REVERSE, SORT,
        },
        matcher,
        osu::ModSelection,
        ApplicationCommandExt, CountryCode, InteractionExt, MessageExt,
    },
    BotResult, Context, Error, Name,
};

use std::sync::Arc;
use twilight_model::{
    application::{
        command::CommandOptionChoice,
        interaction::{application_command::CommandDataOption, ApplicationCommand},
    },
    id::UserId,
};

enum SnipeCommandKind {
    CountryList(CountryListArgs),
    CountryStats(Option<CountryCode>),
    PlayerList(PlayerListArgs),
    PlayerStats(Option<Name>),
    Sniped(Option<Name>),
    SnipeGain(Option<Name>),
    SnipeLoss(Option<Name>),
}

macro_rules! parse_username {
    ($location:literal, $variant:ident, $options:ident, $ctx:ident, $author_id:ident) => {{
        let mut username = None;

        for option in $options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    NAME => username = Some(value.into()),
                    DISCORD => username = parse_discord_option!($ctx, value, $location),
                    _ => bail_cmd_option!($location, string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!($location, integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!($location, boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!($location, subcommand, name)
                }
            }
        }

        let name = match username {
            Some(name) => Some(name),
            None => $ctx.user_config($author_id).await?.osu_username,
        };

        Some(SnipeCommandKind::$variant(name))
    }};
}

impl SnipeCommandKind {
    async fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, String>> {
        let author_id = command.user_id()?;
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => {
                    bail_cmd_option!("snipe", string, name)
                }
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("snipe", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("snipe", boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    COUNTRY => match Self::parse_country(ctx, options)? {
                        Ok(kind_) => kind = Some(kind_),
                        Err(content) => return Ok(Err(content)),
                    },
                    "player" => match Self::parse_player(ctx, options, author_id).await? {
                        Ok(kind_) => kind = Some(kind_),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => bail_cmd_option!("snipe", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions).map(Ok)
    }

    fn parse_country(
        ctx: &Context,
        options: Vec<CommandDataOption>,
    ) -> BotResult<Result<Self, String>> {
        let mut kind = None;

        for option in options {
            match option {
                CommandDataOption::String { name, .. } => {
                    bail_cmd_option!("snipe country", string, name)
                }
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("snipe country", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("snipe country", boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "list" => match parse_country_list(ctx, options)? {
                        Ok(args) => kind = Some(Self::CountryList(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    "stats" => match parse_country_stats(ctx, options)? {
                        Ok(country) => kind = Some(Self::CountryStats(country)),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => bail_cmd_option!("snipe country", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions).map(Ok)
    }

    async fn parse_player(
        ctx: &Context,
        options: Vec<CommandDataOption>,
        author_id: UserId,
    ) -> BotResult<Result<Self, String>> {
        let mut kind = None;

        for option in options {
            match option {
                CommandDataOption::String { name, .. } => {
                    bail_cmd_option!("snipe player", string, name)
                }
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("snipe player", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("snipe player", boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "gain" => {
                        kind = parse_username!(
                            "snipe player gain",
                            SnipeGain,
                            options,
                            ctx,
                            author_id
                        );
                    }
                    "list" => match parse_player_list(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(Self::PlayerList(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    "loss" => {
                        kind = parse_username!(
                            "snipe player loss",
                            SnipeLoss,
                            options,
                            ctx,
                            author_id
                        );
                    }
                    "stats" => {
                        kind = parse_username!(
                            "snipe player stats",
                            PlayerStats,
                            options,
                            ctx,
                            author_id
                        );
                    }
                    "targets" => {
                        kind =
                            parse_username!("snipe player targets", Sniped, options, ctx, author_id)
                    }
                    _ => bail_cmd_option!("snipe player", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions).map(Ok)
    }
}

fn parse_country_list(
    ctx: &Context,
    options: Vec<CommandDataOption>,
) -> BotResult<Result<CountryListArgs, String>> {
    let mut country = None;
    let mut sort = None;

    for option in options {
        match option {
            CommandDataOption::String { name, value } => match name.as_str() {
                COUNTRY => match parse_country_code(ctx, value) {
                    Ok(country_) => country = Some(country_),
                    Err(content) => return Ok(Err(content)),
                },
                SORT => match value.as_str() {
                    "count" => sort = Some(SnipeOrder::Count),
                    "pp" => sort = Some(SnipeOrder::Pp),
                    "stars" => sort = Some(SnipeOrder::Stars),
                    "weighted_pp" => sort = Some(SnipeOrder::WeightedPp),
                    _ => bail_cmd_option!("snipe country list sort", string, value),
                },
                _ => bail_cmd_option!("snipe country list", string, name),
            },
            CommandDataOption::Integer { name, .. } => {
                bail_cmd_option!("snipe country list", integer, name)
            }
            CommandDataOption::Boolean { name, .. } => {
                bail_cmd_option!("snipe country list", boolean, name)
            }
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!("snipe country list", subcommand, name)
            }
        }
    }

    let sort = sort.unwrap_or_default();

    Ok(Ok(CountryListArgs { country, sort }))
}

fn parse_country_stats(
    ctx: &Context,
    options: Vec<CommandDataOption>,
) -> BotResult<Result<Option<CountryCode>, String>> {
    let mut country = None;

    for option in options {
        match option {
            CommandDataOption::String { name, value } => match name.as_str() {
                COUNTRY => match parse_country_code(ctx, value) {
                    Ok(country_) => country = Some(country_),
                    Err(content) => return Ok(Err(content)),
                },
                _ => bail_cmd_option!("snipe country stats", string, name),
            },
            CommandDataOption::Integer { name, .. } => {
                bail_cmd_option!("snipe country stats", integer, name)
            }
            CommandDataOption::Boolean { name, .. } => {
                bail_cmd_option!("snipe country stats", boolean, name)
            }
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!("snipe country stats", subcommand, name)
            }
        }
    }

    Ok(Ok(country))
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

async fn parse_player_list(
    ctx: &Context,
    options: Vec<CommandDataOption>,
    author_id: UserId,
) -> BotResult<Result<PlayerListArgs, String>> {
    let mut config = ctx.user_config(author_id).await?;
    let mut order = None;
    let mut mods = None;
    let mut descending = None;

    for option in options {
        match option {
            CommandDataOption::String { name, value } => match name.as_str() {
                NAME => config.osu_username = Some(value.into()),
                DISCORD => {
                    config.osu_username = parse_discord_option!(ctx, value, "snipe player list")
                }
                SORT => match value.as_str() {
                    ACC => order = Some(SnipeScoreOrder::Accuracy),
                    "len" => order = Some(SnipeScoreOrder::Length),
                    "map_date" => order = Some(SnipeScoreOrder::MapApprovalDate),
                    MISSES => order = Some(SnipeScoreOrder::Misses),
                    "pp" => order = Some(SnipeScoreOrder::Pp),
                    "score_date" => order = Some(SnipeScoreOrder::ScoreDate),
                    "stars" => order = Some(SnipeScoreOrder::Stars),
                    _ => bail_cmd_option!("snipe player list sort", string, value),
                },
                MODS => match matcher::get_mods(&value) {
                    Some(mods_) => mods = Some(mods_),
                    None => match value.parse() {
                        Ok(mods_) => mods = Some(ModSelection::Include(mods_)),
                        Err(_) => return Ok(Err(MODS_PARSE_FAIL.into())),
                    },
                },
                _ => bail_cmd_option!("snipe player list", string, name),
            },
            CommandDataOption::Integer { name, .. } => {
                bail_cmd_option!("snipe player list", integer, name)
            }
            CommandDataOption::Boolean { name, value } => match name.as_str() {
                REVERSE => descending = Some(!value),
                _ => bail_cmd_option!("snipe player list", boolean, name),
            },
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!("snipe player list", subcommand, name)
            }
        }
    }

    let args = PlayerListArgs {
        config,
        order: order.unwrap_or_default(),
        mods,
        descending: descending.unwrap_or(true),
    };

    Ok(Ok(args))
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
