use crate::{util::MessageExt, Args, BotResult, Context};

use std::sync::Arc;
use tokio::time::{self, Duration};
use twilight::model::channel::Message;

#[command]
#[only_guilds()]
#[authority()]
#[short_desc("Prune messages in a channel")]
#[long_desc(
    "Optionally provide a number to delete this \
     many of the latest messages of a channel, defaults to 1. \
     Amount must be between 1 and 99."
)]
#[usage("[number]")]
#[example("3")]
#[aliases("purge")]
async fn prune(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let amount = match args.single::<u64>() {
        Ok(amount) => {
            if amount < 1 || amount > 99 {
                let content = "First argument must be an integer between 1 and 99";
                return msg.respond(&ctx, content).await;
            } else {
                amount + 1
            }
        }
        Err(_) => 2,
    };
    let messages = match ctx
        .http
        .channel_messages(msg.channel_id)
        .limit(amount)
        .unwrap()
        .await
    {
        Ok(msgs) => msgs.into_iter().map(|msg| msg.id).collect::<Vec<_>>(),
        Err(why) => {
            msg.respond(&ctx, "Error while retrieving messages").await?;
            return Err(why.into());
        }
    };
    if let Err(why) = ctx.http.delete_messages(msg.channel_id, messages).await {
        msg.respond(&ctx, "Error while deleting messages").await?;
        return Err(why.into());
    }
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .content(format!("Deleted the last {} messages", amount - 1))?
        .await?;
    time::delay_for(Duration::from_secs(6)).await;
    ctx.http
        .delete_message(response.channel_id, response.id)
        .await?;
    Ok(())
}
