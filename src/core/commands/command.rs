use crate::{Args, BotResult, Context};

use futures::future::BoxFuture;
use std::{fmt, sync::Arc};
use twilight_model::channel::Message;

type BoxResult<'fut> = BoxFuture<'fut, BotResult<()>>;

pub struct Command {
    pub names: &'static [&'static str],
    pub short_desc: &'static str,
    pub long_desc: Option<&'static str>,
    pub usage: Option<&'static str>,
    pub examples: &'static [&'static str],
    pub authority: bool,
    pub owner: bool,
    pub only_guilds: bool,
    pub bucket: Option<&'static str>,
    pub typing: bool,
    pub sub_commands: &'static [&'static Command],
    pub fun:
        for<'fut> fn(Arc<Context>, &'fut Message, Args<'fut>, Option<usize>) -> BoxResult<'fut>,
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Command")
            .field("names", &self.names)
            .field("short_desc", &self.short_desc)
            .field("long_desc", &self.long_desc)
            .field("usage", &self.usage)
            .field("examples", &self.examples)
            .field("sub_commands", &self.sub_commands)
            .finish()
    }
}
