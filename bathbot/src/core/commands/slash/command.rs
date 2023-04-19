use std::sync::Arc;

use twilight_interactions::command::ApplicationCommandData;

use super::CommandResult;
use crate::{
    core::{buckets::BucketName, commands::flags::CommandFlags, Context},
    util::interaction::InteractionCommand,
};

pub struct SlashCommand {
    pub bucket: Option<BucketName>,
    pub create: fn() -> ApplicationCommandData,
    pub exec: fn(Arc<Context>, InteractionCommand) -> CommandResult,
    pub flags: CommandFlags,
}
