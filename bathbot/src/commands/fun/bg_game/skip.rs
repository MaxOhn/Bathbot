use std::sync::Arc;

use bathbot_util::constants::{GENERAL_ISSUE, INVITE_LINK};
use eyre::Result;
use twilight_model::channel::Message;

use crate::{
    core::{buckets::BucketName, commands::checks::check_ratelimit},
    util::ChannelExt,
    Context,
};

pub async fn skip(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    if let Some(cooldown) = check_ratelimit(&ctx, msg.author.id, BucketName::BgSkip).await {
        trace!(
            "Ratelimiting user {} on bucket `BgSkip` for {cooldown} seconds",
            msg.author.id
        );

        let content = format!("Command on cooldown, try again in {cooldown} seconds");
        msg.error(&ctx, content).await?;

        return Ok(());
    }

    let _ = ctx.http.create_typing_trigger(msg.channel_id).await;

    match ctx.bg_games().read(&msg.channel_id).await.get() {
        Some(game) => match game.restart() {
            Ok(_) => {}
            Err(err) => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                return Err(err.wrap_err("Failed to restart game"));
            }
        },
        None => {
            let content = format!(
                "The background guessing game must be started with `/bg`.\n\
                If slash commands are not available in your server, \
                try [re-inviting the bot]({INVITE_LINK})."
            );

            msg.error(&ctx, content).await?;
        }
    }

    Ok(())
}
