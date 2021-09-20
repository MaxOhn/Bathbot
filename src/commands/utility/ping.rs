use crate::{
    commands::SlashCommandBuilder, util::MessageExt, BotResult, CommandData, Context,
    MessageBuilder,
};

use std::{sync::Arc, time::Instant};
use twilight_model::application::{command::Command, interaction::ApplicationCommand};

#[command]
#[short_desc("Check if I'm online")]
#[long_desc(
    "Check if I'm online.\n\
    The latency indicates how fast I receive messages from Discord."
)]
#[aliases("p")]
#[no_typing()]
async fn ping(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let builder = MessageBuilder::new().content("Pong");
    let start = Instant::now();
    let response_raw = data.create_message(&ctx, builder).await?;
    let elapsed = (Instant::now() - start).as_millis();
    let response = response_raw.model().await?;
    let content = format!(":ping_pong: Pong! ({}ms)", elapsed);
    let builder = MessageBuilder::new().content(content);
    response.update_message(&ctx, builder).await?;

    Ok(())
}

pub async fn slash_ping(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    ping(ctx, command.into()).await
}

pub fn slash_ping_command() -> Command {
    SlashCommandBuilder::new("ping", "Check if I'm online").build()
}
