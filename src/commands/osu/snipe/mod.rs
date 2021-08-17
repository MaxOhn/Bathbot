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
    custom_client::SnipeScoreOrder,
    util::{matcher, osu::ModSelection, ApplicationCommandExt, MessageExt},
    BotResult, Context, CountryCode, Error, Name,
};

use std::sync::Arc;
use twilight_model::application::{
    command::{
        BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption,
        CommandOptionChoice, OptionsCommandOptionData,
    },
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

enum SnipeCommandKind {
    CountryList(CountryListArgs),
    CountryStats(CountryCode),
    PlayerList(PlayerListArgs),
    PlayerStats(Option<Name>),
    Sniped(Option<Name>),
    SnipeGain(Option<Name>),
    SnipeLoss(Option<Name>),
}

macro_rules! parse_username {
    ($location:literal, $variant:ident, $options:ident, $ctx:ident) => {{
        let mut username = None;

        for option in $options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "name" => username = Some(value.into()),
                    "discord" => username = parse_discord_option!($ctx, value, $location),
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

        Some(SnipeCommandKind::$variant(username))
    }};
}

impl SnipeCommandKind {
    fn slash(ctx: &Context, command: &mut ApplicationCommand) -> BotResult<Result<Self, String>> {
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
                    "country" => match Self::parse_country(ctx, options)? {
                        Ok(kind_) => kind = Some(kind_),
                        Err(content) => return Ok(Err(content)),
                    },
                    "player" => match Self::parse_player(ctx, options)? {
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

    fn parse_player(
        ctx: &Context,
        options: Vec<CommandDataOption>,
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
                        kind = parse_username!("snipe player gain", SnipeGain, options, ctx);
                    }
                    "list" => match parse_player_list(ctx, options)? {
                        Ok(args) => kind = Some(Self::PlayerList(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    "loss" => {
                        kind = parse_username!("snipe player loss", SnipeLoss, options, ctx);
                    }
                    "stats" => {
                        kind = parse_username!("snipe player stats", PlayerStats, options, ctx);
                    }
                    "targets" => {
                        kind = parse_username!("snipe player targets", Sniped, options, ctx)
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
                "country" => match parse_country_code(ctx, value) {
                    Ok(country_) => country = Some(country_),
                    Err(content) => return Ok(Err(content)),
                },
                "sort" => match value.as_str() {
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
) -> BotResult<Result<CountryCode, String>> {
    let mut country = None;

    for option in options {
        match option {
            CommandDataOption::String { name, value } => match name.as_str() {
                "country" => match parse_country_code(ctx, value) {
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

    country.ok_or(Error::InvalidCommandOptions).map(Ok)
}

fn parse_country_code(ctx: &Context, mut country: String) -> Result<CountryCode, String> {
    match country.as_str() {
        "global" | "world" => Ok("global".into()),
        _ => {
            if country.len() != 2 || !country.is_ascii() {
                let content = format!(
                    "`{}` is not a valid country code.\n\
                    Be sure to specify a two ASCII character country code, e.g. `fr`",
                    country
                );

                return Err(content);
            }

            country.make_ascii_uppercase();

            if !ctx.contains_country(country.as_str()) {
                let content = format!("The country acronym `{}` is not supported :(", country);

                return Err(content);
            }

            Ok(country.into())
        }
    }
}

fn parse_player_list(
    ctx: &Context,
    options: Vec<CommandDataOption>,
) -> BotResult<Result<PlayerListArgs, String>> {
    let mut username = None;
    let mut order = None;
    let mut mods = None;
    let mut descending = None;

    for option in options {
        match option {
            CommandDataOption::String { name, value } => match name.as_str() {
                "name" => username = Some(value.into()),
                "discord" => username = parse_discord_option!(ctx, value, "snipe player list"),
                "sort" => match value.as_str() {
                    "acc" => order = Some(SnipeScoreOrder::Accuracy),
                    "len" => order = Some(SnipeScoreOrder::Length),
                    "map_date" => order = Some(SnipeScoreOrder::MapApprovalDate),
                    "misses" => order = Some(SnipeScoreOrder::Misses),
                    "score_date" => order = Some(SnipeScoreOrder::ScoreDate),
                    "stars" => order = Some(SnipeScoreOrder::Stars),
                    _ => bail_cmd_option!("snipe player list sort", string, value),
                },
                "mods" => match matcher::get_mods(&value) {
                    Some(mods_) => mods = Some(mods_),
                    None => match value.parse() {
                        Ok(mods_) => mods = Some(ModSelection::Exact(mods_)),
                        Err(_) => {
                            let content = "Failed to parse mods.\n\
                            Be sure it's a valid mod abbreviation e.g. `hdhr`.";

                            return Ok(Err(content.into()));
                        }
                    },
                },
                _ => bail_cmd_option!("snipe player list", string, name),
            },
            CommandDataOption::Integer { name, .. } => {
                bail_cmd_option!("snipe player list", integer, name)
            }
            CommandDataOption::Boolean { name, value } => match name.as_str() {
                "reverse" => descending = Some(!value),
                _ => bail_cmd_option!("snipe player list", boolean, name),
            },
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!("snipe player list", subcommand, name)
            }
        }
    }

    let args = PlayerListArgs {
        name: username,
        order: order.unwrap_or_default(),
        mods,
        descending: descending.unwrap_or(true),
    };

    Ok(Ok(args))
}

pub async fn slash_snipe(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match SnipeCommandKind::slash(&ctx, &mut command)? {
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

pub fn slash_snipe_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "snipe".to_owned(),
        default_permission: None,
        description: "National #1 related data provided by huismetbenen".to_owned(),
        id: None,
        options: vec![
            CommandOption::SubCommandGroup(OptionsCommandOptionData {
                description: "Country related snipe stats".to_owned(),
                name: "country".to_owned(),
                options: vec![
                    CommandOption::SubCommand(OptionsCommandOptionData {
                        description: "Sort the country's #1 leaderboard".to_owned(),
                        name: "list".to_owned(),
                        options: vec![
                            CommandOption::String(ChoiceCommandOptionData {
                                choices: vec![],
                                description: "Specify a country".to_owned(),
                                name: "country".to_owned(),
                                required: true,
                            }),
                            CommandOption::String(ChoiceCommandOptionData {
                                choices: vec![
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
                                        name: "weighted_pp".to_owned(),
                                        value: "weighted_pp".to_owned(),
                                    },
                                ],
                                description: "Specify the order of players".to_owned(),
                                name: "sort".to_owned(),
                                required: false,
                            }),
                        ],
                        required: false,
                    }),
                    CommandOption::SubCommand(OptionsCommandOptionData {
                        description: "#1-count related stats for a country".to_owned(),
                        name: "stats".to_owned(),
                        options: vec![CommandOption::String(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Specify a country code".to_owned(),
                            name: "country".to_owned(),
                            required: false,
                        })],
                        required: false,
                    }),
                ],
                required: false,
            }),
            CommandOption::SubCommandGroup(OptionsCommandOptionData {
                description: "Player related snipe stats".to_owned(),
                name: "player".to_owned(),
                options: vec![
                    CommandOption::SubCommand(OptionsCommandOptionData {
                        description: "Display a user's recently acquired national #1 scores"
                            .to_owned(),
                        name: "gain".to_owned(),
                        options: vec![
                            CommandOption::String(ChoiceCommandOptionData {
                                choices: vec![],
                                description: "Specify a username".to_owned(),
                                name: "name".to_owned(),
                                required: false,
                            }),
                            CommandOption::User(BaseCommandOptionData {
                                description: "Specify a linked discord user".to_owned(),
                                name: "discord".to_owned(),
                                required: false,
                            }),
                        ],
                        required: false,
                    }),
                    CommandOption::SubCommand(OptionsCommandOptionData {
                        description: "List all national #1 scores of a player".to_owned(),
                        name: "list".to_owned(),
                        options: vec![
                            CommandOption::String(ChoiceCommandOptionData {
                                choices: vec![],
                                description: "Specify a username".to_owned(),
                                name: "name".to_owned(),
                                required: false,
                            }),
                            CommandOption::String(ChoiceCommandOptionData {
                                choices: vec![],
                                description: "Specify a mods".to_owned(),
                                name: "mods".to_owned(),
                                required: false,
                            }),
                            CommandOption::String(ChoiceCommandOptionData {
                                choices: vec![
                                    CommandOptionChoice::String {
                                        name: "accuracy".to_owned(),
                                        value: "acc".to_owned(),
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
                                        name: "misses".to_owned(),
                                        value: "misses".to_owned(),
                                    },
                                    CommandOptionChoice::String {
                                        name: "score date".to_owned(),
                                        value: "score_date".to_owned(),
                                    },
                                    CommandOptionChoice::String {
                                        name: "stars".to_owned(),
                                        value: "stars".to_owned(),
                                    },
                                ],
                                description: "Specify the order of scores".to_owned(),
                                name: "sort".to_owned(),
                                required: false,
                            }),
                            CommandOption::Boolean(BaseCommandOptionData {
                                description: "Choose whether the list should be reversed"
                                    .to_owned(),
                                name: "reverse".to_owned(),
                                required: false,
                            }),
                            CommandOption::User(BaseCommandOptionData {
                                description: "Specify a linked discord user".to_owned(),
                                name: "discord".to_owned(),
                                required: false,
                            }),
                        ],
                        required: false,
                    }),
                    CommandOption::SubCommand(OptionsCommandOptionData {
                        description: "Display a user's recently lost national #1 scores".to_owned(),
                        name: "loss".to_owned(),
                        options: vec![
                            CommandOption::String(ChoiceCommandOptionData {
                                choices: vec![],
                                description: "Specify a username".to_owned(),
                                name: "name".to_owned(),
                                required: false,
                            }),
                            CommandOption::User(BaseCommandOptionData {
                                description: "Specify a linked discord user".to_owned(),
                                name: "discord".to_owned(),
                                required: false,
                            }),
                        ],
                        required: false,
                    }),
                    CommandOption::SubCommand(OptionsCommandOptionData {
                        description: "Stats about a user's #1 scores in their country leaderboards"
                            .to_owned(),
                        name: "stats".to_owned(),
                        options: vec![
                            CommandOption::String(ChoiceCommandOptionData {
                                choices: vec![],
                                description: "Specify a username".to_owned(),
                                name: "name".to_owned(),
                                required: false,
                            }),
                            CommandOption::User(BaseCommandOptionData {
                                description: "Specify a linked discord user".to_owned(),
                                name: "discord".to_owned(),
                                required: false,
                            }),
                        ],
                        required: false,
                    }),
                    CommandOption::SubCommand(OptionsCommandOptionData {
                        description: "Sniped users of the last 8 weeks".to_owned(),
                        name: "targets".to_owned(),
                        options: vec![
                            CommandOption::String(ChoiceCommandOptionData {
                                choices: vec![],
                                description: "Specify a username".to_owned(),
                                name: "name".to_owned(),
                                required: false,
                            }),
                            CommandOption::User(BaseCommandOptionData {
                                description: "Specify a linked discord user".to_owned(),
                                name: "discord".to_owned(),
                                required: false,
                            }),
                        ],
                        required: false,
                    }),
                ],
                required: false,
            }),
        ],
    }
}
