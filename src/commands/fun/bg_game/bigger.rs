use crate::{
    error::BgGameError,
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use std::sync::Arc;

#[command]
#[short_desc("Increase the size of the image")]
#[aliases("b", "enhance")]
#[bucket("bg_bigger")]
pub(super) async fn bigger(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match ctx.game_bigger(data.channel_id()) {
        Ok(img) => {
            let builder = MessageBuilder::new().file("bg_img.png", &img);
            data.create_message(&ctx, builder).await?;

            Ok(())
        }
        Err(BgGameError::NotStarted) => {
            debug!("Could not get subimage because game didn't start yet");

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
