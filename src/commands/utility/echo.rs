use crate::{commands::checks::*, util::MessageExt};

use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::Message,
    prelude::Context,
    utils::{content_safe, ContentSafeOptions},
};

#[command]
#[checks(Authority)]
#[description = "Make me repeat your message but without any pings"]
#[usage = "[sentence]"]
async fn echo(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let channel = msg.channel_id;
    msg.delete(ctx).await?;
    let content = content_safe(&ctx.cache, args.rest(), &ContentSafeOptions::default()).await;
    channel
        .say(ctx, content)
        .await?
        .reaction_delete(ctx, msg.author.id)
        .await;
    Ok(())
}
