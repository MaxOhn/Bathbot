mod mapper;
mod nochoke;
mod rebalance;
mod top;
mod top_if;
mod top_old;

pub use mapper::*;
pub use nochoke::*;
pub use rebalance::*;
pub use top::*;
pub use top_if::*;
pub use top_old::*;

use super::{prepare_scores, request_user, require_link, ErrorType, GradeArg};

use crate::{
    util::{ApplicationCommandExt, MessageExt},
    BotResult, Context, Error,
};

use std::{borrow::Cow, sync::Arc};
use twilight_model::application::{
    command::{
        BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption,
        CommandOptionChoice, OptionsCommandOptionData,
    },
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

enum TopCommandKind {
    If(IfArgs),
    Mapper(MapperArgs),
    Nochoke(NochokeArgs),
    Old(OldArgs),
    Rebalance(RebalanceArgs),
    Top(TopArgs),
}

impl TopCommandKind {
    async fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let author_id = command.user_id()?;
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => bail_cmd_option!("top", string, name),
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("top", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("top", boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "current" => match TopArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(TopCommandKind::Top(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    "if" => match IfArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(TopCommandKind::If(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    "mapper" => match MapperArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(TopCommandKind::Mapper(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    "nochoke" => match NochokeArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(TopCommandKind::Nochoke(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    "old" => match OldArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(TopCommandKind::Old(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    "rebalance" => match RebalanceArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(TopCommandKind::Rebalance(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => bail_cmd_option!("top", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions).map(Ok)
    }
}

pub async fn slash_top(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match TopCommandKind::slash(&ctx, &mut command).await? {
        Ok(TopCommandKind::If(args)) => _topif(ctx, command.into(), args).await,
        Ok(TopCommandKind::Mapper(args)) => _mapper(ctx, command.into(), args).await,
        Ok(TopCommandKind::Nochoke(args)) => _nochokes(ctx, command.into(), args).await,
        Ok(TopCommandKind::Old(args)) => _topold(ctx, command.into(), args).await,
        Ok(TopCommandKind::Rebalance(args)) => _rebalance(ctx, command.into(), args).await,
        Ok(TopCommandKind::Top(args)) => _top(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

fn _slash_top_current() -> Vec<CommandOption> {
    vec![
        CommandOption::String(ChoiceCommandOptionData {
            choices: super::mode_choices(),
            description: "Specify a mode".to_owned(),
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
                    name: "position".to_owned(),
                    value: "pos".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "date".to_owned(),
                    value: "date".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "accuracy".to_owned(),
                    value: "acc".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "combo".to_owned(),
                    value: "combo".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "length".to_owned(),
                    value: "len".to_owned(),
                },
            ],
            description: "Choose how the scores should be ordered".to_owned(),
            name: "sort".to_owned(),
            required: false,
        }),
        CommandOption::String(ChoiceCommandOptionData {
            choices: vec![],
            description:
                "Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)"
                    .to_owned(),
            name: "mods".to_owned(),
            required: false,
        }),
        CommandOption::Integer(ChoiceCommandOptionData {
            choices: vec![],
            description: "Choose a specific score index between 1 and 100".to_owned(),
            name: "index".to_owned(),
            required: false,
        }),
        CommandOption::User(BaseCommandOptionData {
            description: "Specify a linked discord user".to_owned(),
            name: "discord".to_owned(),
            required: false,
        }),
        CommandOption::Boolean(BaseCommandOptionData {
            description: "Reverse the resulting score list".to_owned(),
            name: "reverse".to_owned(),
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
            ],
            description: "Only scores with this grade".to_owned(),
            name: "grade".to_owned(),
            required: false,
        }),
    ]
}

fn _slash_top_if() -> Vec<CommandOption> {
    vec![
        CommandOption::String(ChoiceCommandOptionData {
            choices: vec![],
            description:
                "Specify mods (`+mods` to insert them, `+mods!` to replace, `-mods!` to remove)"
                    .to_owned(),
            name: "mods".to_owned(),
            required: true,
        }),
        CommandOption::String(ChoiceCommandOptionData {
            choices: vec![
                CommandOptionChoice::String {
                    name: "osu".to_owned(),
                    value: "osu".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "taiko".to_owned(),
                    value: "taiko".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "catch".to_owned(),
                    value: "catch".to_owned(),
                },
            ],
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
    ]
}

fn _slash_top_mapper() -> Vec<CommandOption> {
    vec![
        CommandOption::String(ChoiceCommandOptionData {
            choices: vec![],
            description: "Specify a mapper username".to_owned(),
            name: "mapper".to_owned(),
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
    ]
}

fn _slash_top_nochoke() -> Vec<CommandOption> {
    vec![
        CommandOption::String(ChoiceCommandOptionData {
            choices: vec![
                CommandOptionChoice::String {
                    name: "osu".to_owned(),
                    value: "osu".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "taiko".to_owned(),
                    value: "taiko".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "catch".to_owned(),
                    value: "catch".to_owned(),
                },
            ],
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
        CommandOption::Integer(ChoiceCommandOptionData {
            choices: vec![],
            description: "Only unchoke scores with at most this many misses".to_owned(),
            name: "miss_limit".to_owned(),
            required: false,
        }),
        CommandOption::User(BaseCommandOptionData {
            description: "Specify a linked discord user".to_owned(),
            name: "discord".to_owned(),
            required: false,
        }),
    ]
}

fn _slash_top_old() -> Vec<CommandOption> {
    vec![
        CommandOption::SubCommand(OptionsCommandOptionData {
            description:
                "How the current osu!standard top plays would look like on a previous pp system"
                    .to_owned(),
            name: "osu".to_owned(),
            options: vec![
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![
                        CommandOptionChoice::String {
                            name: "april 2015 - may 2018".to_owned(),
                            value: "april15_may18".to_owned(),
                        },
                        CommandOptionChoice::String {
                            name: "may 2018 - february 2019".to_owned(),
                            value: "may18_february19".to_owned(),
                        },
                        CommandOptionChoice::String {
                            name: "february 2019 - january 2021".to_owned(),
                            value: "february19_january21".to_owned(),
                        },
                        CommandOptionChoice::String {
                            name: "january 2021 - july 2021".to_owned(),
                            value: "january21_july21".to_owned(),
                        },
                    ],
                    description: "Choose which version should replace the current pp system"
                        .to_owned(),
                    name: "version".to_owned(),
                    required: true,
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
        CommandOption::SubCommand(OptionsCommandOptionData {
            description:
                "How the current osu!taiko top plays would look like on a previous pp system"
                    .to_owned(),
            name: "taiko".to_owned(),
            options: vec![
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![CommandOptionChoice::String {
                        name: "march 2014 - september 2020".to_owned(),
                        value: "march14_september20".to_owned(),
                    }],
                    description: "Choose which version should replace the current pp system"
                        .to_owned(),
                    name: "version".to_owned(),
                    required: true,
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
        CommandOption::SubCommand(OptionsCommandOptionData {
            description:
                "How the current osu!catch top plays would look like on a previous pp system"
                    .to_owned(),
            name: "catch".to_owned(),
            options: vec![
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![CommandOptionChoice::String {
                        name: "march 2014 - may 2020".to_owned(),
                        value: "march14_may20".to_owned(),
                    }],
                    description: "Choose which version should replace the current pp system"
                        .to_owned(),
                    name: "version".to_owned(),
                    required: true,
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
        CommandOption::SubCommand(OptionsCommandOptionData {
            description:
                "How the current osu!mania top plays would look like on a previous pp system"
                    .to_owned(),
            name: "mania".to_owned(),
            options: vec![
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![CommandOptionChoice::String {
                        name: "march 2014 - may 2018".to_owned(),
                        value: "march14_may18".to_owned(),
                    }],
                    description: "Choose which version should replace the current pp system"
                        .to_owned(),
                    name: "version".to_owned(),
                    required: true,
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
    ]
}

fn _slash_top_rebalance() -> Vec<CommandOption> {
    vec![
        CommandOption::String(ChoiceCommandOptionData {
            choices: vec![
                CommandOptionChoice::String {
                    name: "delta_t".to_owned(),
                    value: "delta_t".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "sotars".to_owned(),
                    value: "sotarks".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "xexxar".to_owned(),
                    value: "xexxar".to_owned(),
                },
            ],
            description: "Choose which version should replace the current pp system".to_owned(),
            name: "version".to_owned(),
            required: true,
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
    ]
}

pub fn slash_top_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "top".to_owned(),
        default_permission: None,
        description: "Display a user's top plays through various modifications".to_owned(),
        id: None,
        options: vec![
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Count the top plays on maps of the given mapper".to_owned(),
                name: "current".to_owned(),
                options: _slash_top_current(),
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "How the top plays would look like with different mods".to_owned(),
                name: "if".to_owned(),
                options: _slash_top_if(),
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Count the top plays on maps of the given mapper".to_owned(),
                name: "mapper".to_owned(),
                options: _slash_top_mapper(),
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Count the top plays on maps of the given mapper".to_owned(),
                name: "nochoke".to_owned(),
                options: _slash_top_nochoke(),
                required: false,
            }),
            CommandOption::SubCommandGroup(OptionsCommandOptionData {
                description: "How the current top plays would look like on a previous pp system"
                    .to_owned(),
                name: "old".to_owned(),
                options: _slash_top_old(),
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description:
                    "How the current top plays would look like on an alternative pp system"
                        .to_owned(),
                name: "rebalance".to_owned(),
                options: _slash_top_rebalance(),
                required: false,
            }),
        ],
    }
}
