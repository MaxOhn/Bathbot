use crate::{
    util::{constants::GENERAL_ISSUE, MessageExt},
    Args, BotResult, Context,
};

use std::{str::FromStr, sync::Arc};
use tokio::time::{self, Duration};
use twilight_http::{
    api_error::{ApiError, ErrorCode::MessageTooOldToBulkDelete},
    error::ErrorType,
};
use twilight_model::channel::Message;

#[command]
#[only_guilds()]
#[authority()]
#[short_desc("Prune messages in a channel")]
#[long_desc(
    "Optionally provide a number to delete this \
     many of the latest messages of a channel, defaults to 1.\n\
     Amount must be between 1 and 99.\n\
     This command can not delete messages older than 2 weeks."
)]
#[usage("[number]")]
#[example("3")]
#[aliases("purge")]
async fn prune(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let amount = match args.next().map(u64::from_str) {
        Some(Ok(amount)) => {
            if !(1..100).contains(&amount) {
                let content = "First argument must be an integer between 1 and 99";

                return msg.error(&ctx, content).await;
            }

            amount + 1
        }
        None | Some(Err(_)) => 2,
    };

    let msgs_fut = ctx
        .http
        .channel_messages(msg.channel_id)
        .limit(amount)
        .unwrap()
        .exec();

    let mut messages = match msgs_fut.await {
        Ok(msgs) => msgs
            .models()
            .await?
            .into_iter()
            .take(amount as usize)
            .map(|msg| msg.id)
            .collect::<Vec<_>>(),
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why.into());
        }
    };

    if messages.len() < 2 {
        if let Some(msg_id) = messages.pop() {
            ctx.http
                .delete_message(msg.channel_id, msg_id)
                .exec()
                .await?;
        }

        return Ok(());
    }

    match ctx
        .http
        .delete_messages(msg.channel_id, &messages)
        .exec()
        .await
    {
        Ok(_) => {}
        Err(why) => {
            if matches!(why.kind(), ErrorType::Response {
                error: ApiError::General(err),
                ..
            } if err.code == MessageTooOldToBulkDelete)
            {
                let content = "Cannot delete messages that are older than two weeks \\:(";

                return msg.error(&ctx, content).await;
            } else {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                return Err(why.into());
            }
        }
    }

    let response = msg
        .respond(&ctx, format!("Deleted the last {} messages", amount - 1))
        .await?;

    time::sleep(Duration::from_secs(6)).await;

    ctx.http
        .delete_message(response.channel_id, response.id)
        .exec()
        .await?;

    Ok(())
}
