use std::time::Instant;

use bathbot_macros::{SlashCommand, command};
use bathbot_util::MessageBuilder;
use eyre::Result;
use twilight_interactions::command::CreateCommand;
use twilight_model::guild::Permissions;

use crate::{core::commands::CommandOrigin, util::interaction::InteractionCommand};

const PING_DESC: &str = "Check if the bot is online";

const PING_HELP: &str = "Most basic command, generally used to check if the \
bot is online.\nThe displayed latency is the time it takes for the bot to \
receive a response from discord after sending a message.";

#[derive(CreateCommand, SlashCommand)]
#[command(name = "ping", desc = PING_DESC, help = PING_HELP)]
#[flags(SKIP_DEFER)]
pub struct Ping;

async fn slash_ping(mut command: InteractionCommand) -> Result<()> {
    ping((&mut command).into()).await
}

#[command]
#[desc(PING_DESC)]
#[help(PING_HELP)]
#[alias("p")]
#[flags(SKIP_DEFER)]
#[group(Utility)]
async fn prefix_ping(msg: &Message, permissions: Option<Permissions>) -> Result<()> {
    ping(CommandOrigin::from_msg(msg, permissions)).await
}

async fn ping(orig: CommandOrigin<'_>) -> Result<()> {
    let builder = MessageBuilder::new().content("Pong");
    let start = Instant::now();
    let response_raw = orig.callback_with_response(builder).await?;
    let elapsed = (Instant::now() - start).as_millis();

    let response = response_raw.model().await?;

    let response = match orig {
        CommandOrigin::Message { permissions, .. } => {
            CommandOrigin::from_msg(&response, permissions)
        }
        orig @ CommandOrigin::Interaction { .. } => orig,
    };

    let content = format!(":ping_pong: Pong! ({elapsed}ms)");
    let builder = MessageBuilder::new().content(content);
    response.update(builder).await?;

    Ok(())
}
