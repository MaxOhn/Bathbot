use std::sync::Arc;

use crate::{
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, CommandData, Context,
};

use super::GameState;

#[command]
#[bucket("bg_skip")]
#[short_desc("Skip the current background")]
#[aliases("s", "resolve", "r")]
async fn skip(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match ctx.bg_games().get(&data.channel_id()) {
        Some(state) => match state.value() {
            GameState::Running { game } => match game.restart() {
                Ok(_) => Ok(()),
                Err(err) => {
                    let _ = data.error(&ctx, GENERAL_ISSUE).await;

                    Err(err.into())
                }
            },
            GameState::Setup { author, .. } => {
                let content = format!(
                    "The game is currently being setup.\n\
                    <@{author}> must click on the \"Start\" button to begin."
                );

                data.error(&ctx, content).await
            }
        },
        None => {
            // TODO: Put regular msg back it
            let content = "The background guessing game must now be started with `/bg`.\n\
                Everything else stayed as before.";

            // let content = "No running game in this channel. Start one with `/bg`.";

            data.error(&ctx, content).await
        }
    }
}
