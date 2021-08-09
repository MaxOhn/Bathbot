mod track;
mod track_list;
mod untrack;
mod untrack_all;

pub use track::*;
pub use track_list::*;
pub use untrack::*;
pub use untrack_all::*;

use crate::{util::ApplicationCommandExt, Args, BotResult, Context, Error, Name};

use rosu_v2::prelude::GameMode;
use std::sync::Arc;
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
    fn args(
        ctx: &Context,
        args: &mut Args,
        mut limit: Option<usize>,
        mode: Option<GameMode>,
    ) -> Result<Self, &'static str> {
        let mut name = None;
        let mut more_names = Vec::new();

        while let Some(arg) = args
            .next()
            .map(|arg| Args::try_link_name(ctx, arg))
            .transpose()?
        {
            if matches!(arg.as_str(), "-limit" | "-l") {
                match args.next().map(str::parse) {
                    Some(Ok(num)) => limit = Some(num),
                    None | Some(Err(_)) => {
                        return Err("Could not parse given limit, \
                            try specifying a positive number after `-limit`")
                    }
                }
            } else if name.is_none() {
                name = Some(arg);
            } else if more_names.len() < 9 {
                more_names.push(arg);
            }
        }

        Ok(Self {
            name: name.ok_or("You must specify at least one username")?,
            limit,
            more_names,
            mode,
        })
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
                                    "mode" => parse_mode_option!(mode, value, "track add"),
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
                                                            "mode" => parse_mode_option!(
                                                                mode,
                                                                value,
                                                                "track remove user"
                                                            ),
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
                                                            "mode" => parse_mode_option!(
                                                                mode,
                                                                value,
                                                                "track remove all"
                                                            ),
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
    Command {
        application_id: None,
        guild_id: None,
        name: "track".to_owned(),
        default_permission: None,
        description: "(Un)track top score updates for an osu player".to_owned(),
        id: None,
        options: vec![
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
            CommandOption::SubCommand(OptionsCommandOptionData {
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
        ],
    }
}
