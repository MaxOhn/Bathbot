mod pp;
mod score;

pub use pp::*;
pub use score::*;

use super::{request_user, require_link};

use crate::{
    util::{ApplicationCommandExt, MessageExt},
    Args, BotResult, Context, Error, Name,
};

use rosu_v2::prelude::GameMode;
use std::{borrow::Cow, sync::Arc};
use twilight_model::application::{
    command::{
        BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption,
        OptionsCommandOptionData,
    },
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

struct PpArgs {
    name: Option<Name>,
    mode: GameMode,
    country: Option<String>,
    rank: usize,
}

impl PpArgs {
    fn args(ctx: &Context, args: &mut Args<'_>, mode: GameMode) -> Result<Self, &'static str> {
        let mut name = None;
        let mut country = None;
        let mut rank = None;

        for arg in args.take(2) {
            match arg.parse() {
                Ok(num) => rank = Some(num),
                Err(_) => {
                    if arg.len() >= 3 {
                        let (prefix, suffix) = arg.split_at(2);
                        let valid_country = prefix.chars().all(|c| c.is_ascii_alphabetic());

                        if let (true, Ok(num)) = (valid_country, suffix.parse()) {
                            country = Some(prefix.to_owned());
                            rank = Some(num);
                        } else {
                            name = Some(Args::try_link_name(ctx, arg)?);
                        }
                    } else {
                        name = Some(Args::try_link_name(ctx, arg)?);
                    }
                }
            }
        }

        const COUNTRY_PARSE_ERROR: &str =
            "Could not parse rank. Provide it either as positive number \
            or as country acronym followed by a positive number e.g. `be10` \
            as one of the first two arguments.";

        let rank = rank.ok_or(COUNTRY_PARSE_ERROR)?;

        Ok(Self {
            name,
            mode,
            country,
            rank,
        })
    }
}

struct ScoreArgs {
    name: Option<Name>,
    mode: GameMode,
    rank: usize,
}

impl ScoreArgs {
    fn args(ctx: &Context, args: &mut Args<'_>, mode: GameMode) -> Result<Self, &'static str> {
        let mut name = None;
        let mut rank = None;

        for arg in args.take(2) {
            match arg.parse() {
                Ok(num) => rank = Some(num),
                Err(_) => name = Some(Args::try_link_name(ctx, arg)?),
            }
        }

        let rank = rank.ok_or("You must specify a target rank.")?;

        Ok(Self { name, mode, rank })
    }
}

enum RankCommandKind {
    Performance(PpArgs),
    Score(ScoreArgs),
}

impl RankCommandKind {
    fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => bail_cmd_option!("rank", string, name),
                CommandDataOption::Integer { name, .. } => bail_cmd_option!("rank", integer, name),
                CommandDataOption::Boolean { name, .. } => bail_cmd_option!("rank", boolean, name),
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "pp" => {
                        let mut username = None;
                        let mut mode = None;
                        let mut country = None;
                        let mut rank = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "mode" => parse_mode_option!(mode, value, "rank pp"),
                                    "name" => username = Some(value.into()),
                                    "discord" => match value.parse() {
                                        Ok(id) => match ctx.get_link(id) {
                                            Some(name) => username = Some(name),
                                            None => {
                                                let content = format!(
                                                    "<@{}> is not linked to an osu profile",
                                                    id
                                                );

                                                return Ok(Err(content.into()));
                                            }
                                        },
                                        Err(_) => {
                                            bail_cmd_option!("rank pp discord", string, value)
                                        }
                                    },
                                    "country" => country = Some(value),
                                    _ => bail_cmd_option!("rank pp", string, name),
                                },
                                CommandDataOption::Integer { name, value } => match name.as_str() {
                                    "rank" => rank = Some(value.max(0) as usize),
                                    _ => bail_cmd_option!("rank pp", integer, name),
                                },
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("rank pp", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("rank pp", subcommand, name)
                                }
                            }
                        }

                        let rank = rank.ok_or(Error::InvalidCommandOptions)?;
                        let mode = mode.unwrap_or(GameMode::STD);

                        let args = PpArgs {
                            name: username,
                            country,
                            mode,
                            rank,
                        };

                        kind = Some(RankCommandKind::Performance(args));
                    }
                    "score" => {
                        let mut username = None;
                        let mut mode = None;
                        let mut rank = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "mode" => parse_mode_option!(mode, value, "rank pp"),
                                    "name" => username = Some(value.into()),
                                    "discord" => match value.parse() {
                                        Ok(id) => match ctx.get_link(id) {
                                            Some(name) => username = Some(name),
                                            None => {
                                                let content = format!(
                                                    "<@{}> is not linked to an osu profile",
                                                    id
                                                );

                                                return Ok(Err(content.into()));
                                            }
                                        },
                                        Err(_) => {
                                            bail_cmd_option!("rank pp discord", string, value)
                                        }
                                    },
                                    _ => bail_cmd_option!("rank pp", string, name),
                                },
                                CommandDataOption::Integer { name, value } => match name.as_str() {
                                    "rank" => rank = Some(value.max(0) as usize),
                                    _ => bail_cmd_option!("rank pp", integer, name),
                                },
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("rank pp", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("rank pp", subcommand, name)
                                }
                            }
                        }

                        let rank = rank.ok_or(Error::InvalidCommandOptions)?;
                        let mode = mode.unwrap_or(GameMode::STD);

                        let args = ScoreArgs {
                            name: username,
                            mode,
                            rank,
                        };

                        kind = Some(RankCommandKind::Score(args));
                    }
                    _ => bail_cmd_option!("rank", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions).map(Ok)
    }
}

pub async fn slash_rank(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match RankCommandKind::slash(&ctx, &mut command)? {
        Ok(RankCommandKind::Performance(args)) => _rank(ctx, command.into(), args).await,
        Ok(RankCommandKind::Score(args)) => _rankscore(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn slash_rank_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "rank".to_owned(),
        default_permission: None,
        description: "How much is a user missing to reach the given rank?".to_owned(),
        id: None,
        options: vec![
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "How many pp is a user missing to reach the given rank?".to_owned(),
                name: "pp".to_owned(),
                options: vec![
                    CommandOption::Integer(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify the target rank".to_owned(),
                        name: "rank".to_owned(),
                        required: true,
                    }),
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: super::mode_choices(),
                        description: "Specify a gamemode".to_owned(),
                        name: "mode".to_owned(),
                        required: false,
                    }),
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify a username".to_owned(),
                        name: "name".to_owned(),
                        required: false,
                    }),
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify a country code".to_owned(),
                        name: "country".to_owned(),
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
                description: "How much ranked score is a user missing to reach the given rank?"
                    .to_owned(),
                name: "score".to_owned(),
                options: vec![
                    CommandOption::Integer(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify the target rank".to_owned(),
                        name: "rank".to_owned(),
                        required: true,
                    }),
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: super::mode_choices(),
                        description: "Specify a gamemode".to_owned(),
                        name: "mode".to_owned(),
                        required: false,
                    }),
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
    }
}
