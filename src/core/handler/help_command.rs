use crate::{
    core::{Command, MessageExt},
    util::constants::DARK_GREEN,
    BotResult, Context,
};

use std::fmt::Write;
use twilight::{builders::embed::EmbedBuilder, model::channel::Message};

pub async fn help_command(ctx: &Context, cmd: &Command, msg: &Message) -> BotResult<()> {
    todo!()
}
