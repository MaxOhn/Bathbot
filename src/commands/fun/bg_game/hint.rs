use std::sync::Arc;

use crate::{util::MessageExt, BotResult, CommandData, Context, MessageBuilder};

use super::GameState;

#[command]
#[short_desc("Get a hint for the current background")]
#[aliases("h", "tip")]
#[bucket("bg_hint")]
#[no_typing()]
pub(super) async fn hint(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match ctx.bg_games().get(&data.channel_id()) {
        Some(state) => match state.value() {
            GameState::Running { game } => {
                let hint = game.hint().await;
                let builder = MessageBuilder::new().content(hint);
                data.create_message(&ctx, builder).await?;

                Ok(())
            }
            GameState::Setup { author, .. } => {
                let content = format!(
                    "The game is currently being setup.\n\
                    <@{author}> must click on the \"Start\" button to begin."
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
