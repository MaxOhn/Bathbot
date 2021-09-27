pub mod addstream;
pub mod removestream;
pub mod tracked;

pub use addstream::*;
pub use removestream::*;
pub use tracked::*;

use crate::{
    commands::SlashCommandBuilder,
    util::{ApplicationCommandExt, CowUtils},
    Args, BotResult, Context, Error,
};

use std::{borrow::Cow, sync::Arc};
use twilight_model::application::{
    command::{ChoiceCommandOptionData, Command, CommandOption, OptionsCommandOptionData},
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

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
                                    "name" => kind = Some(StreamCommandKind::Add(value)),
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
                                    "name" => kind = Some(StreamCommandKind::Remove(value)),
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

pub fn slash_trackstream_command() -> Command {
    let description = "(Un)track a twitch stream or list all tracked streams in this channel";

    let options = vec![
        CommandOption::SubCommand(OptionsCommandOptionData {
            description: "Track a twitch stream in this channel".to_owned(),
            name: "add".to_owned(),
            options: vec![CommandOption::String(ChoiceCommandOptionData {
                choices: vec![],
                description: "Name of the twitch channel".to_owned(),
                name: "name".to_owned(),
                required: false,
            })],
            required: false,
        }),
        CommandOption::SubCommand(OptionsCommandOptionData {
            description: "Untrack a twitch stream in this channel".to_owned(),
            name: "remove".to_owned(),
            options: vec![CommandOption::String(ChoiceCommandOptionData {
                choices: vec![],
                description: "Name of the twitch channel".to_owned(),
                name: "name".to_owned(),
                required: false,
            })],
            required: false,
        }),
        CommandOption::SubCommand(OptionsCommandOptionData {
            description: "List all tracked twitch stream in this channel".to_owned(),
            name: "list".to_owned(),
            options: vec![],
            required: false,
        }),
    ];

    SlashCommandBuilder::new("trackstream", description)
        .options(options)
        .build()
}
