use std::sync::Arc;

use twilight_model::channel::Message;

use crate::{
    core::{buckets::BucketName, commands::checks::check_ratelimit},
    util::{builder::MessageBuilder, ChannelExt},
    BotResult, Context,
};

use super::GameState;

pub async fn hint(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let ratelimit = check_ratelimit(&ctx, msg.author.id, BucketName::BgHint).await;

    if let Some(cooldown) = ratelimit {
        trace!(
            "Ratelimiting user {} on bucket `BgHint` for {cooldown} seconds",
            msg.author.id
        );

        return Ok(());
    }

    match ctx.bg_games().get(&msg.channel_id) {
        Some(state) => match state.value() {
            GameState::Running { game } => {
                let hint = game.hint().await;
                let builder = MessageBuilder::new().content(hint);
                msg.create_message(&ctx, &builder).await?;
            }
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
