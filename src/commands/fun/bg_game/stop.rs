use crate::{bail, util::MessageExt, Args, BotResult, Context};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Stop the bg game")]
#[aliases("end", "quit")]
pub async fn stop(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    match ctx.stop_game(msg.channel_id).await {
        Ok(true) => Ok(()),
        Ok(false) => {
            let prefix = ctx.config_first_prefix(msg.guild_id);
            let content = format!(
                "No running game in this channel.\nStart one with `{}bg start`.",
                prefix
            );
            msg.error(&ctx, content).await
        }
        Err(why) => {
            let _ = msg.error(&ctx, "Error while stopping game \\:(").await;
            bail!("error while stopping game: {}", why)
        }
    }
}
