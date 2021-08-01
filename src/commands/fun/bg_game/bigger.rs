use crate::{
    util::{constants::GENERAL_ISSUE, error::BgGameError, MessageExt},
    Args, BotResult, Context,
};

use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("Increase the size of the image")]
#[aliases("b", "enhance")]
#[bucket("bg_bigger")]
pub async fn bigger(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    match ctx.game_bigger(msg.channel_id).await {
        Ok(img) => {
            ctx.http
                .create_message(msg.channel_id)
                .files(&[("bg_img.png", &img)])
                .exec()
                .await?;

            Ok(())
        }
        Err(BgGameError::NotStarted) => {
            debug!("Could not get subimage because game didn't start yet");

            Ok(())
        }
        Err(BgGameError::NoGame) => {
            let prefix = ctx.config_first_prefix(msg.guild_id);

            let content = format!(
                "No running game in this channel.\nStart one with `{}bg start`.",
                prefix
            );

            msg.error(&ctx, content).await
        }
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            Err(why.into())
        }
    }
}
