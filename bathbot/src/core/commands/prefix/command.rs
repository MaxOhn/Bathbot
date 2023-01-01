use std::sync::Arc;

use twilight_model::channel::Message;

use crate::core::{buckets::BucketName, commands::flags::CommandFlags, Context};

use super::{Args, CommandResult, PrefixCommandGroup};

pub struct PrefixCommand {
    pub names: &'static [&'static str],
    pub desc: &'static str,
    pub help: Option<&'static str>,
    pub usage: Option<&'static str>,
    pub examples: &'static [&'static str],
    pub bucket: Option<BucketName>,
    pub flags: CommandFlags,
    pub group: PrefixCommandGroup,
    pub exec: for<'f> fn(Arc<Context>, &'f Message, Args<'f>) -> CommandResult<'f>,
}

impl PrefixCommand {
    pub fn name(&self) -> &str {
        self.names[0]
    }
}
