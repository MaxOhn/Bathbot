use std::sync::Arc;

use bathbot_util::{constants::GENERAL_ISSUE, MessageBuilder};
use eyre::Result;
use twilight_model::{channel::Message, guild::Permissions};

use crate::{
    core::{buckets::BucketName, commands::checks::check_ratelimit},
    util::ChannelExt,
    Context,
};

pub async fn hint(
    ctx: Arc<Context>,
    msg: &Message,
    permissions: Option<Permissions>,
) -> Result<()> {
    let ratelimit = check_ratelimit(&ctx, msg.author.id, BucketName::BgHint).await;

    if let Some(cooldown) = ratelimit {
        trace!(
            "Ratelimiting user {} on bucket `BgHint` for {cooldown} seconds",
            msg.author.id
        );

        return Ok(());
    }

    match ctx.bg_games().read(&msg.channel_id).await.get() {
        Some(game) => match game.hint().await {
            Ok(hint) => {
                let builder = MessageBuilder::new().content(hint);
                msg.create_message(&ctx, builder, permissions).await?;
            }
            Err(err) => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                return Err(err.wrap_err("Failed to get hint"));
            }
        },
        None => {
            let content = "No running game in this channel. Start one with `/bg`.";
            msg.error(&ctx, content).await?;
        }
    }

    Ok(())
}
