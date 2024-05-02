use eyre::Result;
use twilight_model::channel::Message;

use crate::{util::ChannelExt, Context};

pub async fn stop(msg: &Message) -> Result<()> {
    match Context::bg_games().read(&msg.channel_id).await.get() {
        Some(game) => match game.stop() {
            Ok(_) => {}
            Err(err) => {
                let _ = msg.error("Error while stopping game \\:(").await;

                return Err(err.wrap_err("Failed to stop game"));
            }
        },
        None => {
            let content = "No running game in this channel. Start one with `/bg`.";
            msg.error(content).await?;
        }
    }

    Ok(())
}
