mod leaderboard;
mod list;
mod score;
mod simulate;

pub use leaderboard::*;
pub use list::*;
pub use score::*;
pub use simulate::*;

use super::{prepare_score, prepare_scores, request_user, require_link, ErrorType, GradeArg};
use crate::{
    util::{osu::ModSelection, ApplicationCommandExt, MessageExt},
    BotResult, Context, Error,
};

use rosu_v2::prelude::{GameMode, Grade};
use std::{borrow::Cow, sync::Arc};
use twilight_model::application::{
    command::{
        BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption,
        CommandOptionChoice, OptionsCommandOptionData,
    },
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

enum RecentCommandKind {
    Leaderboard(RecentLeaderboardArgs),
    List(RecentListArgs),
    Score(RecentArgs),
    Simulate(RecentSimulateArgs),
}

impl RecentCommandKind {
    async fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let author_id = command.user_id()?;
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => bail_cmd_option!("recent", string, name),
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("recent", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("recent", boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "score" => {
                        let mut config = ctx.user_config(author_id).await?;
                        let mut index = None;
                        let mut grade = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "name" => config.name = Some(value.into()),
                                    "discord" => {
                                        config.name =
                                            parse_discord_option!(ctx, value, "recent score")
                                    }
                                    "mode" => {
                                        config.mode = parse_mode_option!(value, "recent score")
                                    }
                                    "grade" => match value.as_str() {
                                        "SS" => {
                                            grade = Some(GradeArg::Range {
                                                bot: Grade::X,
                                                top: Grade::XH,
                                            })
                                        }
                                        "S" => {
                                            grade = Some(GradeArg::Range {
                                                bot: Grade::S,
                                                top: Grade::SH,
                                            })
                                        }
                                        "A" => grade = Some(GradeArg::Single(Grade::A)),
                                        "B" => grade = Some(GradeArg::Single(Grade::B)),
                                        "C" => grade = Some(GradeArg::Single(Grade::C)),
                                        "D" => grade = Some(GradeArg::Single(Grade::D)),
                                        "F" => grade = Some(GradeArg::Single(Grade::F)),
                                        _ => bail_cmd_option!("recent score grade", string, value),
                                    },
                                    _ => bail_cmd_option!("recent score", string, name),
                                },
                                CommandDataOption::Integer { name, value } => match name.as_str() {
                                    "index" => index = Some(value.max(1).min(50) as usize),
                                    _ => bail_cmd_option!("recent score", integer, name),
                                },
                                CommandDataOption::Boolean { name, value } => match name.as_str() {
                                    "passes" => {
                                        if value {
                                            grade = match grade {
                                                Some(GradeArg::Single(Grade::F)) => None,
                                                Some(GradeArg::Single(_)) => grade,
                                                Some(GradeArg::Range { .. }) => grade,
                                                None => Some(GradeArg::Range {
                                                    bot: Grade::D,
                                                    top: Grade::XH,
                                                }),
                                            }
                                        } else {
                                            grade = Some(GradeArg::Single(Grade::F));
                                        }
                                    }
                                    _ => bail_cmd_option!("recent score", boolean, name),
                                },
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("recent score", subcommand, name)
                                }
                            }
                        }

                        let args = RecentArgs {
                            config,
                            index,
                            grade,
                        };

                        kind = Some(RecentCommandKind::Score(args));
                    }
                    "leaderboard" => {
                        let mut config = ctx.user_config(author_id).await?;
                        let mut username = None;
                        let mut mods = None;
                        let mut index = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "name" => username = Some(value.into()),
                                    "mods" => match value.parse() {
                                        Ok(m) => mods = Some(ModSelection::Include(m)),
                                        Err(_) => {
                                            let content = "Failed to parse mods. Be sure to specify a valid abbreviation e.g. hdhr.";

                                            return Ok(Err(content.into()));
                                        }
                                    },
                                    "discord" => {
                                        username =
                                            parse_discord_option!(ctx, value, "recent leaderboard")
                                    }
                                    "mode" => {
                                        config.mode =
                                            parse_mode_option!(value, "recent leaderboard")
                                    }
                                    _ => bail_cmd_option!("recent leaderboard", string, name),
                                },
                                CommandDataOption::Integer { name, value } => match name.as_str() {
                                    "index" => index = Some(value.max(1).min(50) as usize),
                                    _ => bail_cmd_option!("recent leaderboard", integer, name),
                                },
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("recent leaderboard", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("recent leaderboard", subcommand, name)
                                }
                            }
                        }

                        let args = RecentLeaderboardArgs {
                            config,
                            name: username,
                            mods,
                            index,
                        };

                        kind = Some(RecentCommandKind::Leaderboard(args));
                    }
                    "list" => {
                        let mut config = ctx.user_config(author_id).await?;
                        let mut grade = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "name" => config.name = Some(value.into()),
                                    "discord" => {
                                        config.name =
                                            parse_discord_option!(ctx, value, "recent list")
                                    }
                                    "mode" => {
                                        config.mode = parse_mode_option!(value, "recent list")
                                    }
                                    "grade" => match value.as_str() {
                                        "SS" => {
                                            grade = Some(GradeArg::Range {
                                                bot: Grade::X,
                                                top: Grade::XH,
                                            })
                                        }
                                        "S" => {
                                            grade = Some(GradeArg::Range {
                                                bot: Grade::S,
                                                top: Grade::SH,
                                            })
                                        }
                                        "A" => grade = Some(GradeArg::Single(Grade::A)),
                                        "B" => grade = Some(GradeArg::Single(Grade::B)),
                                        "C" => grade = Some(GradeArg::Single(Grade::C)),
                                        "D" => grade = Some(GradeArg::Single(Grade::D)),
                                        "F" => grade = Some(GradeArg::Single(Grade::F)),
                                        _ => bail_cmd_option!("recent list grade", string, value),
                                    },
                                    _ => bail_cmd_option!("recent list", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("recent list", integer, name)
                                }
                                CommandDataOption::Boolean { name, value } => match name.as_str() {
                                    "passes" => {
                                        if value {
                                            grade = match grade {
                                                Some(GradeArg::Single(Grade::F)) => None,
                                                Some(GradeArg::Single(_)) => grade,
                                                Some(GradeArg::Range { .. }) => grade,
                                                None => Some(GradeArg::Range {
                                                    bot: Grade::D,
                                                    top: Grade::XH,
                                                }),
                                            }
                                        } else {
                                            grade = Some(GradeArg::Single(Grade::F));
                                        }
                                    }
                                    _ => bail_cmd_option!("recent list", boolean, name),
                                },
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("recent list", subcommand, name)
                                }
                            }
                        }

                        kind = Some(RecentCommandKind::List(RecentListArgs { config, grade }));
                    }
                    "simulate" => {
                        let mut config = ctx.user_config(author_id).await?;
                        let mut mods = None;
                        let mut index = None;
                        let mut n300 = None;
                        let mut n100 = None;
                        let mut n50 = None;
                        let mut misses = None;
                        let mut acc = None;
                        let mut combo = None;
                        let mut score = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "name" => config.name = Some(value.into()),
                                    "mods" => match value.parse() {
                                        Ok(m) => mods = Some(ModSelection::Include(m)),
                                        Err(_) => {
                                            let content = "Failed to parse mods. Be sure to specify a valid abbreviation e.g. `hdhr`.";

                                            return Ok(Err(content.into()));
                                        }
                                    },
                                    "discord" => {
                                        config.name =
                                            parse_discord_option!(ctx, value, "recent simulate")
                                    }
                                    "mode" => {
                                        config.mode = parse_mode_option!(value, "recent simulate")
                                    }
                                    _ => bail_cmd_option!("recent simulate", string, name),
                                },
                                CommandDataOption::Integer { name, value } => match name.as_str() {
                                    "index" => index = Some(value.max(1).min(50) as usize),
                                    "n300" => n300 = Some(value.max(0) as usize),
                                    "n100" => n100 = Some(value.max(0) as usize),
                                    "n50" => n50 = Some(value.max(0) as usize),
                                    "misses" => misses = Some(value.max(0) as usize),
                                    "combo" => combo = Some(value.max(0) as usize),
                                    "score" => score = Some(value.max(0) as u32),
                                    _ => bail_cmd_option!("recent simulate", integer, name),
                                },
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("recent simulate", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("recent simulate", subcommand, name)
                                }
                            }
                        }

                        let args = RecentSimulateArgs {
                            config,
                            mods,
                            index,
                            n300,
                            n100,
                            n50,
                            misses,
                            acc,
                            combo,
                            score,
                        };

                        kind = Some(RecentCommandKind::Simulate(args));
                    }
                    _ => bail_cmd_option!("recent", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions).map(Ok)
    }
}

pub async fn slash_recent(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match RecentCommandKind::slash(&ctx, &mut command).await? {
        Ok(RecentCommandKind::Score(args)) => _recent(ctx, command.into(), args).await,
        Ok(RecentCommandKind::Leaderboard(args)) => {
            _recentleaderboard(ctx, command.into(), args, false).await
        }
        Ok(RecentCommandKind::List(args)) => _recentlist(ctx, command.into(), args).await,
        Ok(RecentCommandKind::Simulate(args)) => _recentsimulate(ctx, command.into(), args).await,
        Err(msg) => command.error(&ctx, msg).await,
    }
}

pub fn slash_recent_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "recent".to_owned(),
        default_permission: None,
        description: "Display info about a user's recent play".to_owned(),
        id: None,
        options: vec![
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Show a user's recent score".to_owned(),
                name: "score".to_owned(),
                options: vec![
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: super::mode_choices(),
                        description: "Specify the gamemode".to_owned(),
                        name: "mode".to_owned(),
                        required: false,
                    }),
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify a username".to_owned(),
                        name: "name".to_owned(),
                        required: false,
                    }),
                    CommandOption::Integer(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Choose the recent score's index e.g. 1 for most recent"
                            .to_owned(),
                        name: "index".to_owned(),
                        required: false,
                    }),
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: vec![
                            CommandOptionChoice::String {
                                name: "SS".to_owned(),
                                value: "SS".to_owned(),
                            },
                            CommandOptionChoice::String {
                                name: "S".to_owned(),
                                value: "S".to_owned(),
                            },
                            CommandOptionChoice::String {
                                name: "A".to_owned(),
                                value: "A".to_owned(),
                            },
                            CommandOptionChoice::String {
                                name: "B".to_owned(),
                                value: "B".to_owned(),
                            },
                            CommandOptionChoice::String {
                                name: "C".to_owned(),
                                value: "C".to_owned(),
                            },
                            CommandOptionChoice::String {
                                name: "D".to_owned(),
                                value: "D".to_owned(),
                            },
                            CommandOptionChoice::String {
                                name: "F".to_owned(),
                                value: "F".to_owned(),
                            },
                        ],
                        description: "Consider only scores with this grade".to_owned(),
                        name: "grade".to_owned(),
                        required: false,
                    }),
                    CommandOption::Boolean(BaseCommandOptionData {
                        description: "Specify whether only passes should be considered".to_owned(),
                        name: "passes".to_owned(),
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
                description: "Show the leaderboard of a user's recently played map".to_owned(),
                name: "leaderboard".to_owned(),
                options: vec![
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: super::mode_choices(),
                        description: "Specify the gamemode".to_owned(),
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
                        description: "Specify mods e.g. hdhr or nm".to_owned(),
                        name: "mods".to_owned(),
                        required: false,
                    }),
                    CommandOption::Integer(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Choose the recent score's index e.g. 1 for most recent"
                            .to_owned(),
                        name: "index".to_owned(),
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
                description: "Show all recent plays of a user".to_owned(),
                name: "list".to_owned(),
                options: vec![
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: super::mode_choices(),
                        description: "Specify the gamemode".to_owned(),
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
                        choices: vec![
                            CommandOptionChoice::String {
                                name: "SS".to_owned(),
                                value: "SS".to_owned(),
                            },
                            CommandOptionChoice::String {
                                name: "S".to_owned(),
                                value: "S".to_owned(),
                            },
                            CommandOptionChoice::String {
                                name: "A".to_owned(),
                                value: "A".to_owned(),
                            },
                            CommandOptionChoice::String {
                                name: "B".to_owned(),
                                value: "B".to_owned(),
                            },
                            CommandOptionChoice::String {
                                name: "C".to_owned(),
                                value: "C".to_owned(),
                            },
                            CommandOptionChoice::String {
                                name: "D".to_owned(),
                                value: "D".to_owned(),
                            },
                            CommandOptionChoice::String {
                                name: "F".to_owned(),
                                value: "F".to_owned(),
                            },
                        ],
                        description: "Only scores with this grade".to_owned(),
                        name: "grade".to_owned(),
                        required: false,
                    }),
                    CommandOption::Boolean(BaseCommandOptionData {
                        description: "Specify whether the list should include only passes"
                            .to_owned(),
                        name: "passes".to_owned(),
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
                description: "Unchoke a user's recent score or simulate a play on its map"
                    .to_owned(),
                name: "simulate".to_owned(),
                options: vec![
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: super::mode_choices(),
                        description: "Specify the gamemode".to_owned(),
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
                        description: "Specify mods e.g. hdhr or nm".to_owned(),
                        name: "mods".to_owned(),
                        required: false,
                    }),
                    CommandOption::Integer(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Choose the recent score's index e.g. 1 for most recent"
                            .to_owned(),
                        name: "index".to_owned(),
                        required: false,
                    }),
                    CommandOption::Integer(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify the amount of 300s".to_owned(),
                        name: "n300".to_owned(),
                        required: false,
                    }),
                    CommandOption::Integer(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify the amount of 100s".to_owned(),
                        name: "n100".to_owned(),
                        required: false,
                    }),
                    CommandOption::Integer(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify the amount of 50s".to_owned(),
                        name: "n50".to_owned(),
                        required: false,
                    }),
                    CommandOption::Integer(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify the amount of misses".to_owned(),
                        name: "misses".to_owned(),
                        required: false,
                    }),
                    // TODO
                    // CommandOption::Number(ChoiceCommandOptionData {
                    //     choices: vec![],
                    //     description: "Specify the accuracy".to_owned(),
                    //     name: "acc".to_owned(),
                    //     required: false,
                    // }),
                    CommandOption::Integer(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify the combo".to_owned(),
                        name: "combo".to_owned(),
                        required: false,
                    }),
                    CommandOption::Integer(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify the score (only relevant for mania)".to_owned(),
                        name: "score".to_owned(),
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
