use bathbot_util::BucketName;
use twilight_model::{channel::Message, guild::Permissions};

use super::{Args, CommandResult, PrefixCommandGroup};
use crate::core::commands::flags::CommandFlags;

pub struct PrefixCommand {
    pub names: &'static [&'static str],
    pub desc: &'static str,
    pub help: Option<&'static str>,
    pub usage: Option<&'static str>,
    pub examples: &'static [&'static str],
    pub bucket: Option<BucketName>,
    pub flags: CommandFlags,
    pub group: PrefixCommandGroup,
    pub exec: for<'f> fn(&'f Message, Args<'f>, Option<Permissions>) -> CommandResult<'f>,
}

impl PrefixCommand {
    pub fn name(&self) -> &str {
        self.names[0]
    }
}
