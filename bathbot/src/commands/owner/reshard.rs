use std::sync::{Arc, OnceLock};

use bathbot_util::{EmbedBuilder, MessageBuilder};
use eyre::Result;
use tokio::sync::mpsc::Sender;

use crate::{
    core::Context,
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

pub static RESHARD_TX: OnceLock<Sender<()>> = OnceLock::new();

pub async fn reshard(ctx: Arc<Context>, command: InteractionCommand) -> Result<()> {
    RESHARD_TX
        .get()
        .expect("RESHARD_TX has not been initialized")
        .send(())
        .await
        .expect("RESHARD_RX has been dropped");

    let embed = EmbedBuilder::new().description("Reshard message has been sent");
    let builder = MessageBuilder::new().embed(embed);

    command.callback(&ctx, builder, false).await?;

    Ok(())
}
