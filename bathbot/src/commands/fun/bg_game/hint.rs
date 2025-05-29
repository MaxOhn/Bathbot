use bathbot_util::{BucketName, MessageBuilder, constants::GENERAL_ISSUE};
use eyre::Result;
use twilight_model::{channel::Message, guild::Permissions};

use crate::{Context, util::ChannelExt};

pub async fn hint(msg: &Message, permissions: Option<Permissions>) -> Result<()> {
    let ratelimit = Context::check_ratelimit(msg.author.id, BucketName::BgHint);

    if let Some(cooldown) = ratelimit {
        trace!(
            "Ratelimiting user {} on bucket `BgHint` for {cooldown} seconds",
            msg.author.id
        );

        return Ok(());
    }

    match Context::bg_games().read(&msg.channel_id).await.get() {
        Some(game) => match game.hint().await {
            Ok(hint) => {
                let builder = MessageBuilder::new().content(hint);
                msg.create_message(builder, permissions).await?;
            }
            Err(err) => {
                let _ = msg.error(GENERAL_ISSUE).await;

                return Err(err.wrap_err("Failed to get hint"));
            }
        },
        None => {
            let content = "No running game in this channel. Start one with `/bg`.";
            msg.error(content).await?;
        }
    }

    Ok(())
}
