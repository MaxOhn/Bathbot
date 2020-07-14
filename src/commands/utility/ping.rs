use crate::{BotResult, Context};
// use crate::util::MessageExt;

use chrono::Utc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Check if I'm online")]
#[long_desc(
    "Check if I'm online.\n\
    The latency indicates how fast I receive messages from Discord."
)]
#[aliases("p")]
async fn ping(ctx: &Context, msg: &Message) -> BotResult<()> {
    let start = Utc::now().timestamp_millis();
    // let mut response = msg.channel_id.say(ctx, "Pong!").await?;
    // response
    //     .edit(ctx, |m| {
    //         let elapsed = Utc::now().timestamp_millis() - start;
    //         m.content(format!("Pong! ({}ms)", elapsed))
    //     })
    //     .await?;
    // response.reaction_delete(ctx, msg.author.id).await;
    Ok(())
}
