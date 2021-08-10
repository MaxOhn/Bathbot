use crate::{util::MessageExt, BotResult, CommandData, Context};

use std::sync::Arc;

#[command]
#[short_desc("Stop the bg game")]
#[aliases("end", "quit")]
pub(super) async fn stop(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match ctx.stop_game(data.channel_id()).await {
        Ok(true) => Ok(()),
        Ok(false) => {
            let content = "No running game in this channel.\nStart one with `bg start`.";

            data.error(&ctx, content).await
        }
        Err(why) => {
            let _ = data.error(&ctx, "Error while stopping game \\:(").await;

            Err(why)
        }
    }
}
