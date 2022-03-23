use std::sync::Arc;

use crate::{
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use super::BgGameState;

#[command]
#[short_desc("Increase the size of the image")]
#[aliases("b", "enhance")]
#[bucket("bg_bigger")]
pub(super) async fn bigger(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match ctx.bg_games().get(&data.channel_id()) {
        Some(state) => match state.value() {
            BgGameState::Running { game } => match game.sub_image().await {
                Ok(bytes) => {
                    let builder = MessageBuilder::new().file("bg_img.png", bytes);
                    data.create_message(&ctx, builder).await?;

                    Ok(())
                }
                Err(err) => {
                    let _ = data.error(&ctx, GENERAL_ISSUE).await;

                    Err(err.into())
                }
            },
            BgGameState::Setup { author, .. } => {
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
