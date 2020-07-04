use crate::util::MessageExt;

use chrono::Utc;
use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::prelude::Message,
    prelude::Context,
};

#[command]
#[description = "Displaying the current latency between the bot and the discord servers \
(has nothing to do with your own internet connection)"]
#[aliases("p")]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    let start = Utc::now().timestamp_millis();
    let mut response = msg.channel_id.say(ctx, ":ping_pong: Pong!").await?;
    response
        .edit(ctx, |m| {
            let elapsed = Utc::now().timestamp_millis() - start;
            m.content(format!(":ping_pong: Pong! ({}ms)", elapsed))
        })
        .await?;
    response.reaction_delete(ctx, msg.author.id).await;
    Ok(())
}
