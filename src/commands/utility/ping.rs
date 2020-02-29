use crate::util::discord;

use chrono::Utc;
use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::prelude::Message,
    prelude::Context,
};

#[command]
#[description = "Displaying the current latency to the discord servers"]
#[aliases("p")]
fn ping(ctx: &mut Context, msg: &Message) -> CommandResult {
    let start = Utc::now().timestamp_millis();
    let mut response = msg.channel_id.say(&ctx.http, "Pong!")?;
    response.edit(&ctx, |m| {
        let elapsed = Utc::now().timestamp_millis() - start;
        m.content(format!("Pong! ({}ms)", elapsed))
    })?;

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}
