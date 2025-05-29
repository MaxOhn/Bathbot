use bathbot_util::{BucketName, MessageBuilder, constants::GENERAL_ISSUE};
use eyre::Result;
use twilight_model::{channel::Message, guild::Permissions};

use crate::{Context, util::ChannelExt};

pub async fn bigger(msg: &Message, permissions: Option<Permissions>) -> Result<()> {
    if let Some(cooldown) = Context::check_ratelimit(msg.author.id, BucketName::BgBigger) {
        trace!(
            "Ratelimiting user {} on bucket `BgBigger` for {cooldown} seconds",
            msg.author.id
        );

        let content = format!("Command on cooldown, try again in {cooldown} seconds");
        msg.error(content).await?;

        return Ok(());
    }

    let can_attach_files =
        permissions.is_none_or(|permissions| permissions.contains(Permissions::ATTACH_FILES));

    if !can_attach_files {
        let content = "I'm lacking the permission to attach files";
        msg.error(content).await?;

        return Ok(());
    }

    let _ = Context::http().create_typing_trigger(msg.channel_id).await;

    match Context::bg_games().read(&msg.channel_id).await.get() {
        Some(game) => match game.sub_image().await {
            Ok(bytes) => {
                let builder = MessageBuilder::new().attachment("bg_img.png", bytes);
                msg.create_message(builder, permissions).await?;
            }
            Err(err) => {
                let _ = msg.error(GENERAL_ISSUE).await;

                return Err(err.wrap_err("Failed to get subimage"));
            }
        },
        None => {
            let content = "No running game in this channel. Start one with `/bg`.";
            msg.error(content).await?;
        }
    }

    Ok(())
}
