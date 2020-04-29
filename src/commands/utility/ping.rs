use crate::util::discord;

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
async fn ping(ctx: &mut Context, msg: &Message) -> CommandResult {
    let start = Utc::now().timestamp_millis();
    let mut response = msg.channel_id.say(&ctx.http, "Pong!").await?;
    response
        .edit(&ctx, |m| {
            let elapsed = Utc::now().timestamp_millis() - start;
            m.content(format!("Pong! ({}ms)", elapsed))
        })
        .await?;

    discord::reaction_deletion(&ctx, response, msg.author.id).await;
    Ok(())
}
