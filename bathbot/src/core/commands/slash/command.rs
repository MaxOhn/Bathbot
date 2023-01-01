use std::sync::Arc;

use twilight_interactions::command::ApplicationCommandData;

use crate::{
    core::{buckets::BucketName, commands::flags::CommandFlags, Context},
    util::interaction::InteractionCommand,
};

use super::CommandResult;

pub struct SlashCommand {
    pub bucket: Option<BucketName>,
    pub create: fn() -> ApplicationCommandData,
    pub exec: fn(Arc<Context>, InteractionCommand) -> CommandResult,
    pub flags: CommandFlags,
}
