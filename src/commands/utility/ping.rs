use crate::{
    commands::MyCommand, util::MessageExt, BotResult, CommandData, Context, MessageBuilder,
};

use std::{sync::Arc, time::Instant};
use twilight_model::application::interaction::ApplicationCommand;

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
    let content = format!(":ping_pong: Pong! ({elapsed}ms)");
    let builder = MessageBuilder::new().content(content);
    response.update_message(&ctx, builder).await?;

    Ok(())
}

pub async fn slash_ping(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    ping(ctx, command.into()).await
}

pub fn define_ping() -> MyCommand {
    let help = "Most basic command, generally used to check if the bot is online.\n\
        The displayed latency is the time it takes for the bot \
        to receive a response from discord after sending a message.";

    MyCommand::new("ping", "Check if I'm online").help(help)
}
