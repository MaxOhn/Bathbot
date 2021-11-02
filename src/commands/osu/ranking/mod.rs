mod countries;
mod players;

pub use countries::*;
pub use players::*;

use crate::{
    commands::{
        osu::{option_country, option_mode},
        parse_mode_option, MyCommand, MyCommandOption,
    },
    util::{
        constants::common_literals::{COUNTRY, MODE, SCORE},
        CountryCode, MessageExt,
    },
    BotResult, Context, Error,
};

use rosu_v2::prelude::GameMode;
use std::sync::Arc;
use twilight_model::application::interaction::{
    application_command::{CommandDataOption, CommandOptionValue},
    ApplicationCommand,
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
    fn slash_pp(options: &[CommandDataOption]) -> BotResult<Result<Self, String>> {
        let mut mode = None;
        let mut country = None;

        for option in options {
            match &option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    MODE => mode = parse_mode_option(value),
                    COUNTRY => {
                        if value.len() == 2 && value.is_ascii() {
                            country = Some(value.as_str().into())
                        } else if let Some(code) = CountryCode::from_name(value.as_str()) {
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
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        let mode = mode.unwrap_or(GameMode::STD);

        Ok(Ok(RankingCommandKind::Performance { country, mode }))
    }

    fn parse_mode(options: &[CommandDataOption]) -> BotResult<GameMode> {
        let mode = match options.first() {
            Some(option) => match &option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    MODE => parse_mode_option(value),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            },
            None => None,
        };

        Ok(mode.unwrap_or(GameMode::STD))
    }

    fn slash(command: &mut ApplicationCommand) -> BotResult<Result<Self, String>> {
        let option = command
            .data
            .options
            .first()
            .ok_or(Error::InvalidCommandOptions)?;

        match &option.value {
            CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                "pp" => Self::slash_pp(options),
                SCORE => Ok(Ok(RankingCommandKind::RankedScore {
                    mode: Self::parse_mode(options)?,
                })),
                COUNTRY => Ok(Ok(RankingCommandKind::Country {
                    mode: Self::parse_mode(options)?,
                })),
                _ => Err(Error::InvalidCommandOptions),
            },
            _ => Err(Error::InvalidCommandOptions),
        }
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
