mod countries;
mod players;

pub use countries::*;
pub use players::*;

use crate::{
    commands::{
        osu::{option_country, option_mode},
        MyCommand, MyCommandOption,
    },
    util::{
        constants::common_literals::{COUNTRY, MODE, SCORE},
        ApplicationCommandExt, CountryCode, MessageExt,
    },
    BotResult, Context, Error,
};

use rosu_v2::prelude::GameMode;
use std::sync::Arc;
use twilight_model::application::interaction::{
    application_command::CommandDataOption, ApplicationCommand,
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

const RANKING: &str = "ranking";
const RANKING_PP: &str = "ranking pp";
const RANKING_SCORE: &str = "ranking score";
const RANKING_COUNTRY: &str = "ranking country";

impl RankingCommandKind {
    fn slash(command: &mut ApplicationCommand) -> BotResult<Result<Self, String>> {
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => {
                    bail_cmd_option!(RANKING, string, name)
                }
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!(RANKING, integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!(RANKING, boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "pp" => {
                        let mut mode = None;
                        let mut country = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    MODE => mode = parse_mode_option!(value, "ranking pp"),
                                    COUNTRY => {
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
                                    _ => bail_cmd_option!(RANKING_PP, string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!(RANKING_PP, integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!(RANKING_PP, boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!(RANKING_PP, subcommand, name)
                                }
                            }
                        }

                        let mode = mode.unwrap_or(GameMode::STD);
                        kind = Some(RankingCommandKind::Performance { country, mode });
                    }
                    SCORE => {
                        let mut mode = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    MODE => mode = parse_mode_option!(value, "ranking score"),
                                    _ => bail_cmd_option!(RANKING_SCORE, string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!(RANKING_SCORE, integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!(RANKING_SCORE, boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!(RANKING_SCORE, subcommand, name)
                                }
                            }
                        }

                        let mode = mode.unwrap_or(GameMode::STD);
                        kind = Some(RankingCommandKind::RankedScore { mode });
                    }
                    COUNTRY => {
                        let mut mode = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    MODE => mode = parse_mode_option!(value, "ranking country"),
                                    _ => bail_cmd_option!(RANKING_COUNTRY, string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!(RANKING_COUNTRY, integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!(RANKING_COUNTRY, boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!(RANKING_COUNTRY, subcommand, name)
                                }
                            }
                        }

                        let mode = mode.unwrap_or(GameMode::STD);
                        kind = Some(RankingCommandKind::Country { mode });
                    }
                    _ => bail_cmd_option!(RANKING, subcommand, name),
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

pub fn define_ranking() -> MyCommand {
    let mode = option_mode();

    let country = option_country();

    let pp_help = "Display the global or country based performance points leaderboard";

    let pp = MyCommandOption::builder("pp", "Show the pp ranking")
        .help(pp_help)
        .subcommand(vec![mode, country]);

    let mode = option_mode();

    let score_help = "Display the global ranked score leaderboard";

    let score = MyCommandOption::builder(SCORE, "Show the ranked score ranking")
        .help(score_help)
        .subcommand(vec![mode]);

    let mode = option_mode();

    let country_help = "Display the country leaderboard based on accumulated pp";

    let country = MyCommandOption::builder(COUNTRY, "Show the country ranking")
        .help(country_help)
        .subcommand(vec![mode]);

    MyCommand::new("ranking", "Show the pp, ranked score, or country ranking")
        .options(vec![pp, score, country])
}
