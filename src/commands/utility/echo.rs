use crate::{
    util::{content_safe, MessageExt},
    Args, BotResult, Context,
};

use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[only_guilds()]
#[authority()]
#[short_desc("Let me repeat your message")]
#[long_desc("Let me repeat your message but without any mentions")]
#[usage("[sentence]")]
async fn echo(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let channel = msg.channel_id;
    ctx.http.delete_message(channel, msg.id).await?;
    let mut content = args.rest().to_owned();
    content_safe(&ctx, &mut content, msg.guild_id);

    ctx.http
        .create_message(msg.channel_id)
        .content(content)?
        .await?
        .reaction_delete(&ctx, msg.author.id);

    Ok(())
}
