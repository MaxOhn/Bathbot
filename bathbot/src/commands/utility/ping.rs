use std::{sync::Arc, time::Instant};

use bathbot_macros::{command, SlashCommand};
use bathbot_util::MessageBuilder;
use eyre::{ContextCompat, Result};
use twilight_interactions::command::CreateCommand;
use twilight_model::guild::Permissions;

use crate::{
    core::{commands::CommandOrigin, Context},
    util::{interaction::InteractionCommand, CheckPermissions, MessageExt},
};

#[derive(CreateCommand, SlashCommand)]
#[command(
    name = "ping",
    help = "Most basic command, generally used to check if the bot is online.\n\
    The displayed latency is the time it takes for the bot \
    to receive a response from discord after sending a message."
)]
#[flags(SKIP_DEFER)]
/// Check if the bot is online
pub struct Ping;

async fn slash_ping(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    ping(ctx, (&mut command).into()).await
}

#[command]
#[desc("Check if the bot is online")]
#[help(
    "Most basic command, generally used to check if the bot is online.\n\
    The displayed latency is the time it takes for the bot \
    to receive a response from discord after sending a message."
)]
#[alias("p")]
#[flags(SKIP_DEFER)]
#[group(Utility)]
async fn prefix_ping(
    ctx: Arc<Context>,
    msg: &Message,
    permissions: Option<Permissions>,
) -> Result<()> {
    ping(ctx, CommandOrigin::from_msg(msg, permissions)).await
}

async fn ping(ctx: Arc<Context>, orig: CommandOrigin<'_>) -> Result<()> {
    let builder = MessageBuilder::new().content("Pong");
    let start = Instant::now();
    let response_raw = orig.callback_with_response(&ctx, builder).await?;
    let elapsed = (Instant::now() - start).as_millis();

    let response = response_raw.model().await?;
    let content = format!(":ping_pong: Pong! ({elapsed}ms)");
    let builder = MessageBuilder::new().content(content);

    response
        .update(&ctx, &builder, orig.permissions())
        .wrap_err("lacking permission to update message")?
        .await?;

    Ok(())
}
