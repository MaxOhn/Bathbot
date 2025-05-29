use bathbot_util::{
    BucketName,
    constants::{GENERAL_ISSUE, INVITE_LINK},
};
use eyre::Result;
use twilight_model::channel::Message;

use crate::{Context, util::ChannelExt};

pub async fn skip(msg: &Message) -> Result<()> {
    if let Some(cooldown) = Context::check_ratelimit(msg.author.id, BucketName::BgSkip) {
        trace!(
            "Ratelimiting user {} on bucket `BgSkip` for {cooldown} seconds",
            msg.author.id
        );

        let content = format!("Command on cooldown, try again in {cooldown} seconds");
        msg.error(content).await?;

        return Ok(());
    }

    let _ = Context::http().create_typing_trigger(msg.channel_id).await;

    match Context::bg_games().read(&msg.channel_id).await.get() {
        Some(game) => match game.restart() {
            Ok(_) => {}
            Err(err) => {
                let _ = msg.error(GENERAL_ISSUE).await;

                return Err(err.wrap_err("Failed to restart game"));
            }
        },
        None => {
            let content = format!(
                "The background guessing game must be started with `/bg`.\n\
                If slash commands are not available in your server, \
                try [re-inviting the bot]({INVITE_LINK})."
            );

            msg.error(content).await?;
        }
    }

    Ok(())
}
