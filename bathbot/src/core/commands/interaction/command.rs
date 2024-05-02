use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    sync::OnceLock,
};

use twilight_interactions::command::ApplicationCommandData;
use twilight_model::id::{marker::CommandMarker, Id};

use super::{twilight_command::Command, CommandResult};
use crate::{
    core::{buckets::BucketName, commands::flags::CommandFlags},
    util::interaction::InteractionCommand,
};

#[derive(Copy, Clone)]
pub enum InteractionCommandKind {
    Chat(&'static SlashCommand),
    Message(&'static MessageCommand),
}

impl InteractionCommandKind {
    pub fn create(&self) -> Command {
        match self {
            InteractionCommandKind::Chat(cmd) => (cmd.create)().into(),
            InteractionCommandKind::Message(cmd) => (cmd.create)(),
        }
    }

    pub fn flags(&self) -> CommandFlags {
        match self {
            InteractionCommandKind::Chat(cmd) => cmd.flags,
            InteractionCommandKind::Message(cmd) => cmd.flags,
        }
    }

    pub fn id(&self) -> Id<CommandMarker> {
        match self {
            InteractionCommandKind::Chat(cmd) => *cmd.id.get().expect("missing command id"),
            InteractionCommandKind::Message(cmd) => *cmd.id.get().expect("missing command id"),
        }
    }

    pub fn mention<'n>(&self, name: &'n str) -> CommandMention<'n> {
        CommandMention {
            id: self.id(),
            name,
        }
    }
}

pub struct SlashCommand {
    pub bucket: Option<BucketName>,
    pub create: fn() -> ApplicationCommandData,
    pub exec: fn(InteractionCommand) -> CommandResult,
    pub flags: CommandFlags,
    pub name: &'static str,
    pub id: OnceLock<Id<CommandMarker>>,
}

pub struct MessageCommand {
    pub create: fn() -> Command,
    pub exec: fn(InteractionCommand) -> CommandResult,
    pub flags: CommandFlags,
    pub name: &'static str,
    pub id: OnceLock<Id<CommandMarker>>,
}

pub struct CommandMention<'n> {
    id: Id<CommandMarker>,
    name: &'n str,
}

impl Display for CommandMention<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let Self { id, name } = self;
        write!(f, "</{name}:{id}>")
    }
}
