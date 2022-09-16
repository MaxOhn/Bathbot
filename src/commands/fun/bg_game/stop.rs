use std::sync::Arc;

use eyre::Result;
use twilight_model::channel::Message;

use crate::{util::ChannelExt, Context};

use super::GameState;

pub async fn stop(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    match ctx.bg_games().read(&msg.channel_id).await.get() {
        Some(GameState::Running { game }) => match game.stop() {
            Ok(_) => {}
            Err(err) => {
                let _ = msg.error(&ctx, "Error while stopping game \\:(").await;

                return Err(err.wrap_err("failed to stop game"));
            }
        },
        Some(GameState::Setup { author, .. }) => {
            let content = format!(
                "The game is currently being setup.\n\
                Only <@{author}> can click on the \"Cancel\" button to abort."
            );

            msg.error(&ctx, content).await?;
        }
        None => {
            let content = "No running game in this channel. Start one with `/bg`.";
            msg.error(&ctx, content).await?;
        }
    }

    Ok(())
}
