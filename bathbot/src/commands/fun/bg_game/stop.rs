use std::sync::Arc;

use eyre::Result;
use twilight_model::channel::Message;

use crate::{util::ChannelExt, Context};

pub async fn stop(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    match ctx.bg_games().read(&msg.channel_id).await.get() {
        Some(game) => match game.stop() {
            Ok(_) => {}
            Err(err) => {
                let _ = msg.error(&ctx, "Error while stopping game \\:(").await;

                return Err(err.wrap_err("Failed to stop game"));
            }
        },
        None => {
            let content = "No running game in this channel. Start one with `/bg`.";
            msg.error(&ctx, content).await?;
        }
    }

    Ok(())
}
