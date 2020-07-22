use crate::{
    util::{content_safe, MessageExt},
    BotResult, Context,
};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[only_guilds()]
// #[checks(Authority)]
#[short_desc("Let me repeat your message")]
#[long_desc("Let me repeat your message but without any pings")]
#[usage("[sentence]")]
async fn echo(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let channel = msg.channel_id;
    ctx.http.delete_message(channel, msg.id).await?;
    let mut content = msg.content.clone();
    content_safe(&ctx, &mut content, msg.guild_id);
    msg.respond(&ctx, content).await?;
    Ok(())
}
