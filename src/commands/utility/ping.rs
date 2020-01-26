use chrono::Utc;
use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::prelude::Message,
    prelude::Context,
};

#[command]
#[aliases("p")]
fn ping(ctx: &mut Context, msg: &Message) -> CommandResult {
    let start = Utc::now().timestamp_millis();
    let mut msg = msg.channel_id.say(&ctx.http, "Pong!")?;
    msg.edit(&ctx, |m| {
        let elapsed = Utc::now().timestamp_millis() - start;
        m.content(format!("Pong! ({}ms)", elapsed));
        m
    })?;
    Ok(())
}
