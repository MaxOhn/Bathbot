mod track;
mod track_list;
mod untrack;
mod untrack_all;

pub use track::*;
pub use track_list::*;
pub use untrack::*;
pub use untrack_all::*;

use crate::{
    commands::SlashCommandBuilder,
    util::{ApplicationCommandExt, CowUtils},
    Args, BotResult, Context, Error, Name,
};

use rosu_v2::prelude::GameMode;
use std::{borrow::Cow, sync::Arc};
use twilight_model::application::{
    command::{
        ChoiceCommandOptionData, Command, CommandOption, CommandOptionChoice,
        OptionsCommandOptionData,
    },
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

pub async fn slash_track(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match TrackArgs::slash(&mut command)? {
        TrackCommandKind::Add(args) => _track(ctx, command.into(), args).await,
        TrackCommandKind::RemoveAll(mode) => _untrackall(ctx, command.into(), mode).await,
        TrackCommandKind::RemoveSpecific(args) => _untrack(ctx, command.into(), args).await,
        TrackCommandKind::List => tracklist(ctx, command.into()).await,
    }
}

struct TrackArgs {
    mode: Option<GameMode>,
    name: Name,
    limit: Option<usize>,
    more_names: Vec<Name>,
}

enum TrackCommandKind {
    Add(TrackArgs),
    RemoveAll(Option<GameMode>),
    RemoveSpecific(TrackArgs),
    List,
}

impl TrackArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        mut limit: Option<usize>,
        mode: Option<GameMode>,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut name = None;
        let mut more_names = Vec::new();

        for arg in args.map(CowUtils::cow_to_ascii_lowercase) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "limit" | "l" => match value.parse() {
                        Ok(num) => limit = Some(num),
                        Err(_) => {
                            let content = "Failed to parse `limit`. Must be either an integer.";

                            return Ok(Err(content.into()));
                        }
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `limit`.",
                            key
                        );

                        return Ok(Err(content.into()));
                    }
                }
            } else {
                let name_ = match Args::check_user_mention(ctx, arg.as_ref()).await? {
                    Ok(name) => name,
                    Err(content) => return Ok(Err(content.into())),
                };

                if name.is_none() {
                    name = Some(name_);
                } else if more_names.len() < 9 {
                    more_names.push(name_);
                }
            }
        }

        let name = match name {
            Some(name) => name,
            None => return Ok(Err("You must specify at least one username".into())),
        };

        let args = Self {
            name,
            limit,
            more_names,
            mode,
        };

        Ok(Ok(args))
    }

    fn slash(command: &mut ApplicationCommand) -> BotResult<TrackCommandKind> {
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => bail_cmd_option!("track", string, name),
                CommandDataOption::Integer { name, .. } => bail_cmd_option!("track", integer, name),
                CommandDataOption::Boolean { name, .. } => bail_cmd_option!("track", boolean, name),
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "add" => {
                        let mut mode = None;
                        let mut username = None;
                        let mut limit = None;
                        let mut more_names = Vec::new();

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "mode" => mode = parse_mode_option!(value, "track add"),
                                    "name" => username = Some(value.into()),
                                    _ if name.starts_with("name") => more_names.push(value.into()),
                                    _ => bail_cmd_option!("track add", string, name),
                                },
                                CommandDataOption::Integer { name, value } => match name.as_str() {
                                    "limit" => limit = Some(value.max(1).min(100) as usize),
                                    _ => bail_cmd_option!("track add", integer, name),
                                },
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("track add", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("track add", subcommand, name)
                                }
                            }
                        }

                        let args = TrackArgs {
                            name: username.ok_or(Error::InvalidCommandOptions)?,
                            mode: Some(mode.ok_or(Error::InvalidCommandOptions)?),
                            more_names,
                            limit,
                        };

                        kind = Some(TrackCommandKind::Add(args));
                    }
                    "remove" => {
                        for option in options {
                            match option {
                                CommandDataOption::String { name, .. } => {
                                    bail_cmd_option!("track remove", string, name)
                                }
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("track remove", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("track remove", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, options } => {
                                    match name.as_str() {
                                        "user" => {
                                            let mut mode = None;
                                            let mut username = None;
                                            let mut more_names = Vec::new();

                                            for option in options {
                                                match option {
                                                    CommandDataOption::String { name, value } => {
                                                        match name.as_str() {
                                                            "mode" => {
                                                                mode = parse_mode_option!(
                                                                    value,
                                                                    "track remove user"
                                                                )
                                                            }
                                                            "name" => username = Some(value.into()),
                                                            _ if name.starts_with("name") => {
                                                                more_names.push(value.into())
                                                            }
                                                            _ => bail_cmd_option!(
                                                                "track remove user",
                                                                string,
                                                                name
                                                            ),
                                                        }
                                                    }
                                                    CommandDataOption::Integer { name, .. } => {
                                                        bail_cmd_option!(
                                                            "track remove user",
                                                            integer,
                                                            name
                                                        )
                                                    }
                                                    CommandDataOption::Boolean { name, .. } => {
                                                        bail_cmd_option!(
                                                            "track remove user",
                                                            boolean,
                                                            name
                                                        )
                                                    }
                                                    CommandDataOption::SubCommand {
                                                        name, ..
                                                    } => {
                                                        bail_cmd_option!(
                                                            "track remove user",
                                                            subcommand,
                                                            name
                                                        )
                                                    }
                                                }
                                            }

                                            let args = TrackArgs {
                                                name: username
                                                    .ok_or(Error::InvalidCommandOptions)?,
                                                mode,
                                                more_names,
                                                limit: None,
                                            };

                                            kind = Some(TrackCommandKind::RemoveSpecific(args));
                                        }
                                        "all" => {
                                            let mut mode = None;

                                            for option in options {
                                                match option {
                                                    CommandDataOption::String { name, value } => {
                                                        match name.as_str() {
                                                            "mode" => {
                                                                mode = parse_mode_option!(
                                                                    value,
                                                                    "track remove all"
                                                                )
                                                            }
                                                            _ => bail_cmd_option!(
                                                                "track remove all",
                                                                string,
                                                                name
                                                            ),
                                                        }
                                                    }
                                                    CommandDataOption::Integer { name, .. } => {
                                                        bail_cmd_option!(
                                                            "track remove all",
                                                            integer,
                                                            name
                                                        )
                                                    }
                                                    CommandDataOption::Boolean { name, .. } => {
                                                        bail_cmd_option!(
                                                            "track remove all",
                                                            boolean,
                                                            name
                                                        )
                                                    }
                                                    CommandDataOption::SubCommand {
                                                        name, ..
                                                    } => {
                                                        bail_cmd_option!(
                                                            "track remove all",
                                                            subcommand,
                                                            name
                                                        )
                                                    }
                                                }
                                            }

                                            kind = Some(TrackCommandKind::RemoveAll(mode));
                                        }
                                        _ => bail_cmd_option!("track remove", subcommand, name),
                                    }
                                }
                            }
                        }
                    }
                    "list" => kind = Some(TrackCommandKind::List),
                    _ => bail_cmd_option!("track", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions)
    }
}

pub fn slash_track_command() -> Command {
    let description = "(Un)track top score updates for an osu player";

    let options = vec![
        CommandOption::SubCommand(OptionsCommandOptionData {
            description: "Track a player in a channel".to_owned(),
            name: "add".to_owned(),
            options: vec![
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Choose a username to be tracked".to_owned(),
                    name: "name".to_owned(),
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
                        CommandOptionChoice::String {
                            name: "mania".to_owned(),
                            value: "mania".to_owned(),
                        },
                    ],
                    description: "Specify a mode for the tracked user(s)".to_owned(),
                    name: "mode".to_owned(),
                    required: true,
                }),
                CommandOption::Integer(ChoiceCommandOptionData {
                    choices: vec![],
                    description:
                        "Between 1-100, default 50, notify on updates of the users top X scores"
                            .to_owned(),
                    name: "limit".to_owned(),
                    required: false,
                }),
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Choose a second username to be tracked".to_owned(),
                    name: "name2".to_owned(),
                    required: false,
                }),
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Choose a third username to be tracked".to_owned(),
                    name: "name3".to_owned(),
                    required: false,
                }),
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Choose a fourth username to be tracked".to_owned(),
                    name: "name4".to_owned(),
                    required: false,
                }),
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Choose a fifth username to be tracked".to_owned(),
                    name: "name5".to_owned(),
                    required: false,
                }),
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Choose a sixth username to be tracked".to_owned(),
                    name: "name6".to_owned(),
                    required: false,
                }),
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Choose a seventh username to be tracked".to_owned(),
                    name: "name7".to_owned(),
                    required: false,
                }),
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Choose an eighth username to be tracked".to_owned(),
                    name: "name8".to_owned(),
                    required: false,
                }),
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Choose a ninth username to be tracked".to_owned(),
                    name: "name9".to_owned(),
                    required: false,
                }),
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Choose a tenth username to be tracked".to_owned(),
                    name: "name10".to_owned(),
                    required: false,
                }),
            ],
            required: false,
        }),
        CommandOption::SubCommandGroup(OptionsCommandOptionData {
            description: "Untrack a player in a channel".to_owned(),
            name: "remove".to_owned(),
            options: vec![
                CommandOption::SubCommand(OptionsCommandOptionData {
                    description: "Untrack specific players in a channel".to_owned(),
                    name: "user".to_owned(),
                    options: vec![
                        CommandOption::String(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Choose a username to be untracked".to_owned(),
                            name: "name".to_owned(),
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
                                CommandOptionChoice::String {
                                    name: "mania".to_owned(),
                                    value: "mania".to_owned(),
                                },
                            ],
                            description: "Specify a mode for the tracked user(s)".to_owned(),
                            name: "mode".to_owned(),
                            required: false,
                        }),
                        CommandOption::String(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Choose a second username to be untracked".to_owned(),
                            name: "name2".to_owned(),
                            required: false,
                        }),
                        CommandOption::String(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Choose a third username to be untracked".to_owned(),
                            name: "name3".to_owned(),
                            required: false,
                        }),
                        CommandOption::String(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Choose a fourth username to be untracked".to_owned(),
                            name: "name4".to_owned(),
                            required: false,
                        }),
                        CommandOption::String(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Choose a fifth username to be untracked".to_owned(),
                            name: "name5".to_owned(),
                            required: false,
                        }),
                        CommandOption::String(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Choose a sixth username to be untracked".to_owned(),
                            name: "name6".to_owned(),
                            required: false,
                        }),
                        CommandOption::String(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Choose a seventh username to be untracked".to_owned(),
                            name: "name7".to_owned(),
                            required: false,
                        }),
                        CommandOption::String(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Choose an eighth username to be untracked".to_owned(),
                            name: "name8".to_owned(),
                            required: false,
                        }),
                        CommandOption::String(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Choose a ninth username to be untracked".to_owned(),
                            name: "name9".to_owned(),
                            required: false,
                        }),
                        CommandOption::String(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Choose a tenth username to be untracked".to_owned(),
                            name: "name10".to_owned(),
                            required: false,
                        }),
                    ],
                    required: false,
                }),
                CommandOption::SubCommand(OptionsCommandOptionData {
                    description: "Untrack all players in a channel".to_owned(),
                    name: "all".to_owned(),
                    options: vec![CommandOption::String(ChoiceCommandOptionData {
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
                            CommandOptionChoice::String {
                                name: "mania".to_owned(),
                                value: "mania".to_owned(),
                            },
                        ],
                        description: "Specify a mode for the tracked users".to_owned(),
                        name: "mode".to_owned(),
                        required: false,
                    })],
                    required: false,
                }),
            ],
            required: false,
        }),
        CommandOption::SubCommand(OptionsCommandOptionData {
            description: "List all tracked player in this channel".to_owned(),
            name: "list".to_owned(),
            options: vec![],
            required: false,
        }),
    ];

    SlashCommandBuilder::new("track", description)
        .options(options)
        .build()
}
