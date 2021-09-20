use crate::{
    util::{constants::GENERAL_ISSUE, error::BgGameError, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use std::sync::Arc;

#[command]
#[short_desc("Get a hint for the current background")]
#[aliases("h", "tip")]
#[bucket("bg_hint")]
#[no_typing()]
pub(super) async fn hint(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match ctx.game_hint(data.channel_id()) {
        Ok(hint) => {
            let builder = MessageBuilder::new().content(hint);
            data.create_message(&ctx, builder).await?;

            Ok(())
        }
        Err(BgGameError::NotStarted) => {
            debug!("Could not get hint because game didn't start yet");

            Ok(())
        }
        Err(BgGameError::NoGame) => {
            let content = "No running game in this channel.\nStart one with `bg start`.";

            data.error(&ctx, content).await
        }
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            Err(why.into())
        }
    }
}
