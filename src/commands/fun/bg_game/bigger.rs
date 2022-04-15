use std::sync::Arc;

use twilight_model::channel::Message;

use crate::{
    core::{buckets::BucketName, commands::checks::check_ratelimit},
    util::{builder::MessageBuilder, constants::GENERAL_ISSUE, ChannelExt},
    BotResult, Context,
};

use super::GameState;

pub async fn bigger(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    if let Some(cooldown) = check_ratelimit(&ctx, msg.author.id, BucketName::BgBigger).await {
        trace!(
            "Ratelimiting user {} on bucket `BgBigger` for {cooldown} seconds",
            msg.author.id
        );

        let content = format!("Command on cooldown, try again in {cooldown} seconds");
        msg.error(&ctx, content).await?;

        return Ok(());
    }

    let _ = ctx.http.create_typing_trigger(msg.channel_id).exec().await;

    match ctx.bg_games().get(&msg.channel_id) {
        Some(state) => match state.value() {
            GameState::Running { game } => match game.sub_image().await {
                Ok(bytes) => {
                    let builder = MessageBuilder::new().attachment("bg_img.png", bytes);
                    msg.create_message(&ctx, &builder).await?;
                }
                Err(err) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err.into());
                }
            },
            GameState::Setup { author, .. } => {
                let content = format!(
                    "The game is currently being setup.\n\
                    <@{author}> must click on the \"Start\" button to begin."
                );

                msg.error(&ctx, content).await?;
            }
        },
        None => {
            let content = "No running game in this channel. Start one with `/bg`.";
            msg.error(&ctx, content).await?;
        }
    }

    Ok(())
}
