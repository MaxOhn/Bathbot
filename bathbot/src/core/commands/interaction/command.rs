use std::sync::Arc;

use twilight_interactions::command::ApplicationCommandData;
use twilight_model::application::command::Command;

use super::CommandResult;
use crate::{
    core::{buckets::BucketName, commands::flags::CommandFlags, Context},
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
}

pub struct SlashCommand {
    pub bucket: Option<BucketName>,
    pub create: fn() -> ApplicationCommandData,
    pub exec: fn(Arc<Context>, InteractionCommand) -> CommandResult,
    pub flags: CommandFlags,
    pub name: &'static str,
}

pub struct MessageCommand {
    pub create: fn() -> Command,
    pub exec: fn(Arc<Context>, InteractionCommand) -> CommandResult,
    pub flags: CommandFlags,
    pub name: &'static str,
}
