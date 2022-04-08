use std::sync::Arc;

use twilight_interactions::command::ApplicationCommandData;
use twilight_model::application::interaction::ApplicationCommand;

use crate::core::{buckets::BucketName, commands::flags::CommandFlags, Context};

use super::CommandResult;

pub struct SlashCommand {
    pub bucket: Option<BucketName>,
    pub create: fn() -> ApplicationCommandData,
    pub exec: fn(Arc<Context>, Box<ApplicationCommand>) -> CommandResult,
    pub flags: CommandFlags,
}
