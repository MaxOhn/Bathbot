pub mod addstream;
pub mod removestream;
pub mod tracked;

use std::{borrow::Cow, sync::Arc};

use twilight_model::application::interaction::{
    application_command::CommandOptionValue, ApplicationCommand,
};

use crate::{
    util::{constants::common_literals::NAME, CowUtils},
    Args, BotResult, Context, Error,
};

pub use self::{addstream::*, removestream::*, tracked::*};

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
        command
            .data
            .options
            .pop()
            .and_then(|option| match option.value {
                CommandOptionValue::SubCommand(mut options) => match option.name.as_str() {
                    "add" => options
                        .pop()
                        .filter(|option| option.name == NAME)
                        .and_then(|option| match option.value {
                            CommandOptionValue::String(value) => Some(value),
                            _ => None,
                        })
                        .map(StreamCommandKind::Add),
                    "list" => Some(StreamCommandKind::List),
                    "remove" => options
                        .pop()
                        .filter(|option| option.name == NAME)
                        .and_then(|option| match option.value {
                            CommandOptionValue::String(value) => Some(value),
                            _ => None,
                        })
                        .map(StreamCommandKind::Remove),
                    _ => None,
                },
                _ => None,
            })
            .ok_or(Error::InvalidCommandOptions)
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
