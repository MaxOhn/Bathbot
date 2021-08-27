mod countries;
mod players;

pub use countries::*;
pub use players::*;

use crate::{
    util::{ApplicationCommandExt, CountryCode, MessageExt},
    BotResult, Context, Error,
};

use rosu_v2::prelude::GameMode;
use std::sync::Arc;
use twilight_model::application::{
    command::{ChoiceCommandOptionData, Command, CommandOption, OptionsCommandOptionData},
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

enum RankingCommandKind {
    Country {
        mode: GameMode,
    },
    Performance {
        country: Option<CountryCode>,
        mode: GameMode,
    },
    RankedScore {
        mode: GameMode,
    },
}

impl RankingCommandKind {
    fn slash(command: &mut ApplicationCommand) -> BotResult<Result<Self, String>> {
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => {
                    bail_cmd_option!("ranking", string, name)
                }
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("ranking", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("ranking", boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "pp" => {
                        let mut mode = None;
                        let mut country = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "mode" => mode = parse_mode_option!(value, "ranking pp"),
                                    "country" => {
                                        if value.len() == 2 && value.is_ascii() {
                                            country = Some(value.into())
                                        } else if let Some(code) =
                                            CountryCode::from_name(value.as_str())
                                        {
                                            country = Some(code)
                                        } else {
                                            let content = format!(
                                                "Failed to parse `{}` as country.\n\
                                                Be sure to specify a valid country or two ASCII letter country code.",
                                                value
                                            );

                                            return Ok(Err(content));
                                        }
                                    }
                                    _ => bail_cmd_option!("ranking pp", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("ranking pp", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("ranking pp", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("ranking pp", subcommand, name)
                                }
                            }
                        }

                        let mode = mode.unwrap_or(GameMode::STD);
                        kind = Some(RankingCommandKind::Performance { country, mode });
                    }
                    "score" => {
                        let mut mode = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "mode" => mode = parse_mode_option!(value, "ranking score"),
                                    _ => bail_cmd_option!("ranking score", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("ranking score", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("ranking score", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("ranking score", subcommand, name)
                                }
                            }
                        }

                        let mode = mode.unwrap_or(GameMode::STD);
                        kind = Some(RankingCommandKind::RankedScore { mode });
                    }
                    "country" => {
                        let mut mode = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "mode" => mode = parse_mode_option!(value, "ranking country"),
                                    _ => bail_cmd_option!("ranking country", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("ranking country", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("ranking country", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("ranking country", subcommand, name)
                                }
                            }
                        }

                        let mode = mode.unwrap_or(GameMode::STD);
                        kind = Some(RankingCommandKind::Country { mode });
                    }
                    _ => bail_cmd_option!("ranking", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions).map(Ok)
    }
}

pub async fn slash_ranking(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match RankingCommandKind::slash(&mut command)? {
        Ok(RankingCommandKind::Country { mode }) => {
            _countryranking(ctx, command.into(), mode).await
        }
        Ok(RankingCommandKind::Performance { country, mode }) => {
            _performanceranking(ctx, command.into(), mode, country).await
        }
        Ok(RankingCommandKind::RankedScore { mode }) => {
            _scoreranking(ctx, command.into(), mode).await
        }
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn slash_ranking_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "ranking".to_owned(),
        default_permission: None,
        description: "Show the pp, ranked score, or country ranking".to_owned(),
        id: None,
        options: vec![
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Show the pp ranking".to_owned(),
                name: "pp".to_owned(),
                options: vec![
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: super::mode_choices(),
                        description: "Specify the gamemode".to_owned(),
                        name: "mode".to_owned(),
                        required: false,
                    }),
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify a country (code)".to_owned(),
                        name: "country".to_owned(),
                        required: false,
                    }),
                ],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Show the ranked score ranking".to_owned(),
                name: "score".to_owned(),
                options: vec![CommandOption::String(ChoiceCommandOptionData {
                    choices: super::mode_choices(),
                    description: "Specify the gamemode".to_owned(),
                    name: "mode".to_owned(),
                    required: false,
                })],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Show the country ranking".to_owned(),
                name: "country".to_owned(),
                options: vec![CommandOption::String(ChoiceCommandOptionData {
                    choices: super::mode_choices(),
                    description: "Specify the gamemode".to_owned(),
                    name: "mode".to_owned(),
                    required: false,
                })],
                required: false,
            }),
        ],
    }
}
