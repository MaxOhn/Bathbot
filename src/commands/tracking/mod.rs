mod track;
mod track_list;
mod untrack;
mod untrack_all;

pub use track::*;
pub use track_list::*;
pub use untrack::*;
pub use untrack_all::*;

use crate::{
    util::{
        constants::common_literals::{CTB, MANIA, MODE, NAME, OSU, TAIKO},
        ApplicationCommandExt, CowUtils,
    },
    Args, BotResult, Context, Error, Name,
};

use rosu_v2::prelude::GameMode;
use std::{borrow::Cow, sync::Arc};
use twilight_model::application::{
    command::CommandOptionChoice,
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

use super::{MyCommand, MyCommandOption};

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

    fn slash_remove(options: Vec<CommandDataOption>) -> BotResult<TrackCommandKind> {
        let mut kind = None;

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
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "user" => {
                        let mut mode = None;
                        let mut username = None;
                        let mut more_names = Vec::new();

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    MODE => mode = parse_mode_option!(value, "track remove user"),
                                    NAME => username = Some(value.into()),
                                    _ if name.starts_with(NAME) => more_names.push(value.into()),
                                    _ => bail_cmd_option!("track remove user", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("track remove user", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("track remove user", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("track remove user", subcommand, name)
                                }
                            }
                        }

                        let args = TrackArgs {
                            name: username.ok_or(Error::InvalidCommandOptions)?,
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
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    MODE => mode = parse_mode_option!(value, "track remove all"),
                                    _ => bail_cmd_option!("track remove all", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("track remove all", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("track remove all", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("track remove all", subcommand, name)
                                }
                            }
                        }

                        kind = Some(TrackCommandKind::RemoveAll(mode));
                    }
                    _ => bail_cmd_option!("track remove", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions)
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
                                    MODE => mode = parse_mode_option!(value, "track add"),
                                    NAME => username = Some(value.into()),
                                    _ if name.starts_with(NAME) => more_names.push(value.into()),
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
                    "remove" => kind = Some(Self::slash_remove(options)?),
                    "list" => kind = Some(TrackCommandKind::List),
                    _ => bail_cmd_option!("track", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions)
    }
}

fn option_names() -> Vec<MyCommandOption> {
    let name2 =
        MyCommandOption::builder("name2", "Specify a second username").string(Vec::new(), false);

    let name3 =
        MyCommandOption::builder("name3", "Specify a third username").string(Vec::new(), false);

    let name4 =
        MyCommandOption::builder("name4", "Specify a fourth username").string(Vec::new(), false);

    let name5 =
        MyCommandOption::builder("name5", "Specify a fifth username").string(Vec::new(), false);

    let name6 =
        MyCommandOption::builder("name6", "Specify a sixth username").string(Vec::new(), false);

    let name7 =
        MyCommandOption::builder("name7", "Specify a seventh username").string(Vec::new(), false);

    let name8 =
        MyCommandOption::builder("name8", "Specify a eighth username").string(Vec::new(), false);

    let name9 =
        MyCommandOption::builder("name9", "Specify a ninth username").string(Vec::new(), false);

    let name10 =
        MyCommandOption::builder("name10", "Specify a tenth username").string(Vec::new(), false);

    vec![
        name2, name3, name4, name5, name6, name7, name8, name9, name10,
    ]
}

fn option_mode() -> MyCommandOption {
    MyCommandOption::builder("mode", "Specify a mode for the tracked users").string(
        vec![
            CommandOptionChoice::String {
                name: OSU.to_owned(),
                value: OSU.to_owned(),
            },
            CommandOptionChoice::String {
                name: TAIKO.to_owned(),
                value: TAIKO.to_owned(),
            },
            CommandOptionChoice::String {
                name: CTB.to_owned(),
                value: CTB.to_owned(),
            },
            CommandOptionChoice::String {
                name: MANIA.to_owned(),
                value: MANIA.to_owned(),
            },
        ],
        false,
    )
}

fn subcommand_add() -> MyCommandOption {
    let name =
        MyCommandOption::builder(NAME, "Choose a username to be tracked").string(Vec::new(), true);

    let mode = option_mode();

    let limit_description =
        "Between 1-100, default 50, notify on updates of the user's top X scores";

    let limit_help =
        "If not specified, updates in the user's top50 will trigger notification messages.\n\
        Instead of the top50, this `limit` option allows to adjust the maximum index within \
        the top scores.\nThe value must be between 1 and 100.";

    let limit = MyCommandOption::builder("limit", limit_description)
        .help(limit_help)
        .integer(Vec::new(), false);

    let mut options = vec![name, mode, limit];
    options.append(&mut option_names());

    let help = "Add users to the tracking list for this channel.\n\
        If a tracked user gets a new top score, this channel will be notified about it.";

    MyCommandOption::builder("add", "Track top scores of a player")
        .help(help)
        .subcommand(options)
}

fn subcommand_remove() -> MyCommandOption {
    let name = MyCommandOption::builder(NAME, "Choose a username to be untracked")
        .string(Vec::new(), true);

    let mode = option_mode();
    let mut options = vec![name, mode];
    options.append(&mut option_names());

    let user =
        MyCommandOption::builder("user", "Untrack specific users in a channel").subcommand(options);

    let mode = option_mode();
    let all =
        MyCommandOption::builder("all", "Untrack all users in a channel").subcommand(vec![mode]);

    let help =
        "Untrack players in a channel i.e. stop sending notifications when they get new top scores";

    MyCommandOption::builder("remove", "Untrack players in a channel")
        .help(help)
        .subcommandgroup(vec![user, all])
}

fn subcommand_list() -> MyCommandOption {
    MyCommandOption::builder("list", "List all players that are tracked in this channel")
        .subcommand(Vec::new())
}

pub fn define_track() -> MyCommand {
    let options = vec![subcommand_add(), subcommand_remove(), subcommand_list()];

    MyCommand::new("track", "(Un)track top score updates for an osu! player")
        .options(options)
        .authority()
}
