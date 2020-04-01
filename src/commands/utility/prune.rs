use crate::commands::checks::*;

use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::{thread, time::Duration};

#[command]
#[checks(Authority)]
#[description = "Optionally provide a number to delete this \
                 many of the latest messages of a channel, defaults to 1. \
                 Amount must be between 1 and 99."]
#[usage = "[number]"]
#[example = "3"]
#[aliases("purge")]
async fn prune(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let amount = if args.remaining() > 0 {
        match args.trimmed().single::<u64>() {
            Ok(val) => {
                if val < 1 || val > 99 {
                    msg.channel_id
                        .say(
                            &ctx.http,
                            "First argument must be an integer between 1 and 99",
                        )
                        .await?;
                    return Ok(());
                } else {
                    val + 1
                }
            }
            Err(_) => {
                msg.channel_id
                    .say(
                        &ctx.http,
                        "First argument must be a number between 1 and 99",
                    )
                    .await?;
                return Ok(());
            }
        }
    } else {
        2
    };
    let messages = msg
        .channel_id
        .messages(&ctx.http, |retriever| retriever.limit(amount))
        .await?;
    msg.channel_id.delete_messages(&ctx.http, messages).await?;
    let msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.content(format!("Deleted the last {} messages", amount - 1))
        })
        .await?;
    thread::sleep(Duration::from_secs(6));
    msg.delete(&ctx).await?;
    Ok(())
}
