use std::sync::OnceLock;

use bathbot_util::{EmbedBuilder, MessageBuilder};
use eyre::Result;
use tokio::sync::broadcast::Sender;

use crate::util::{InteractionCommandExt, interaction::InteractionCommand};

pub static RESHARD_TX: OnceLock<Sender<()>> = OnceLock::new();

pub async fn reshard(command: InteractionCommand) -> Result<()> {
    RESHARD_TX
        .get()
        .expect("RESHARD_TX has not been initialized")
        .send(())
        .expect("RESHARD_RX has been dropped");

    let embed = EmbedBuilder::new().description("Reshard message has been sent");
    let builder = MessageBuilder::new().embed(embed);

    command.callback(builder, false).await?;

    Ok(())
}
