use std::sync::Arc;

use twilight_model::channel::Message;

use crate::{
    core::{buckets::BucketName, commands::checks::check_ratelimit},
    util::{
        constants::{GENERAL_ISSUE, INVITE_LINK},
        ChannelExt,
    },
    BotResult, Context,
};

use super::GameState;

pub async fn skip(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    if let Some(cooldown) = check_ratelimit(&ctx, msg.author.id, BucketName::BgSkip).await {
        trace!(
            "Ratelimiting user {} on bucket `BgSkip` for {cooldown} seconds",
            msg.author.id
        );

        let content = format!("Command on cooldown, try again in {cooldown} seconds");
        msg.error(&ctx, content).await?;

        return Ok(());
    }

    let _ = ctx.http.create_typing_trigger(msg.channel_id).exec().await;

    match ctx.bg_games().get(&msg.channel_id) {
        Some(state) => match state.value() {
            GameState::Running { game } => match game.restart() {
                Ok(_) => {}
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
