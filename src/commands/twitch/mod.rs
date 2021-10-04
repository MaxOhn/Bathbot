pub mod addstream;
pub mod removestream;
pub mod tracked;

pub use addstream::*;
pub use removestream::*;
pub use tracked::*;

use crate::{
    util::{constants::common_literals::NAME, ApplicationCommandExt, CowUtils},
    Args, BotResult, Context, Error,
};

use std::{borrow::Cow, sync::Arc};
use twilight_model::application::interaction::{
    application_command::CommandDataOption, ApplicationCommand,
};

use super::{MyCommand, MyCommandOption};

enum StreamCommandKind {
    Add(String),
    Remove(String),
    List,
}

struct StreamArgs;

impl StreamArgs {
    fn args<'n>(args: &mut Args<'n>) -> Result<Cow<'n, str>, &'static str> {
        match args.next() {
            Some(arg) => Ok(arg.cow_to_ascii_lowercase()),
            None => Err("The first argument must be the name of the stream"),
        }
    }

    fn slash(command: &mut ApplicationCommand) -> BotResult<StreamCommandKind> {
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => {
                    bail_cmd_option!("trackstream", string, name)
                }
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("trackstream", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("trackstream", boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "add" => {
                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    NAME => kind = Some(StreamCommandKind::Add(value)),
                                    _ => bail_cmd_option!("trackstream add", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("trackstream add", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("trackstream add", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("trackstream add", subcommand, name)
                                }
                            }
                        }
                    }
                    "remove" => {
                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    NAME => kind = Some(StreamCommandKind::Remove(value)),
                                    _ => bail_cmd_option!("trackstream remove", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("trackstream remove", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("trackstream remove", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("trackstream remove", subcommand, name)
                                }
                            }
                        }
                    }
                    "list" => kind = Some(StreamCommandKind::List),
                    _ => bail_cmd_option!("trackstream", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions)
    }
}

pub async fn slash_trackstream(
    ctx: Arc<Context>,
    mut command: ApplicationCommand,
) -> BotResult<()> {
    match StreamArgs::slash(&mut command)? {
        StreamCommandKind::Add(name) => _addstream(ctx, command.into(), name.as_str()).await,
        StreamCommandKind::Remove(name) => _removestream(ctx, command.into(), name.as_str()).await,
        StreamCommandKind::List => tracked(ctx, command.into()).await,
    }
}

fn option_name() -> MyCommandOption {
    MyCommandOption::builder(NAME, "Name of the twitch channel").string(Vec::new(), true)
}

fn subcommand_add() -> MyCommandOption {
    let help = "Track a twitch stream in this channel.\n\
        When the stream goes online, a notification will be send to this channel within a few minutes.";

    MyCommandOption::builder("add", "Track a twitch stream in this channel")
        .help(help)
        .subcommand(vec![option_name()])
}

fn subcommand_remove() -> MyCommandOption {
    MyCommandOption::builder("remove", "Untrack a twitch stream in this channel")
        .subcommand(vec![option_name()])
}

fn subcommand_list() -> MyCommandOption {
    MyCommandOption::builder("list", "List all tracked twitch stream in this channel")
        .subcommand(Vec::new())
}

pub fn define_trackstream() -> MyCommand {
    let options = vec![subcommand_add(), subcommand_remove(), subcommand_list()];
    let description = "(Un)track a twitch stream or list all tracked streams in this channel";

    MyCommand::new("trackstream", description)
        .options(options)
        .authority()
}
