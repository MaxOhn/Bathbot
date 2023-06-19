use std::sync::Arc;

use bathbot_util::{constants::GENERAL_ISSUE, MessageBuilder};
use eyre::Result;
use twilight_model::{channel::Message, guild::Permissions};

use crate::{
    core::{buckets::BucketName, commands::checks::check_ratelimit},
    util::ChannelExt,
    Context,
};

pub async fn bigger(
    ctx: Arc<Context>,
    msg: &Message,
    permissions: Option<Permissions>,
) -> Result<()> {
    if let Some(cooldown) = check_ratelimit(&ctx, msg.author.id, BucketName::BgBigger).await {
        trace!(
            "Ratelimiting user {} on bucket `BgBigger` for {cooldown} seconds",
            msg.author.id
        );

        let content = format!("Command on cooldown, try again in {cooldown} seconds");
        msg.error(&ctx, content).await?;

        return Ok(());
    }

    let can_attach_files = permissions.map_or(true, |permissions| {
        permissions.contains(Permissions::ATTACH_FILES)
    });

    if !can_attach_files {
        let content = "I'm lacking the permission to attach files";
        msg.error(&ctx, content).await?;

        return Ok(());
    }

    let _ = ctx.http.create_typing_trigger(msg.channel_id).await;

    match ctx.bg_games().read(&msg.channel_id).await.get() {
        Some(game) => match game.sub_image().await {
            Ok(bytes) => {
                let builder = MessageBuilder::new().attachment("bg_img.png", bytes);
                msg.create_message(&ctx, builder, permissions).await?;
            }
            Err(err) => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                return Err(err.wrap_err("Failed to get subimage"));
            }
        },
        None => {
            let content = "No running game in this channel. Start one with `/bg`.";
            msg.error(&ctx, content).await?;
        }
    }

    Ok(())
}
