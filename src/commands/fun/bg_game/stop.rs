use crate::{util::MessageExt, BotResult, CommandData, Context};

use std::sync::Arc;

use super::GameState;

#[command]
#[short_desc("Stop the bg game")]
#[aliases("end", "quit")]
pub(super) async fn stop(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match ctx.bg_games().get(&data.channel_id()) {
        Some(state) => match state.value() {
            GameState::Running { game } => match game.stop() {
                Ok(_) => Ok(()),
                Err(err) => {
                    let _ = data.error(&ctx, "Error while stopping game \\:(").await;

                    Err(err.into())
                }
            },
            GameState::Setup { author, .. } => {
                let content = format!(
                    "The game is currently being setup.\n\
                    Only <@{author}> can click on the \"Cancel\" button to abort."
                );

                data.error(&ctx, content).await
            }
        },
        None => {
            let content = "No running game in this channel. Start one with `/bg`.";

            data.error(&ctx, content).await
        }
    }
}
