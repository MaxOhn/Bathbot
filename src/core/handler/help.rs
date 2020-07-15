use crate::{
    core::{CommandGroups, MessageExt},
    util::constants::DARK_GREEN,
    BotResult, Context,
};

use std::fmt::Write;
use twilight::{builders::embed::EmbedBuilder, model::channel::Message};

pub async fn help(ctx: &Context, cmds: &CommandGroups, msg: &Message) -> BotResult<()> {
    todo!()
}
