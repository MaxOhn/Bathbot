use crate::{
    util::{content_safe, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use std::sync::Arc;

#[command]
#[only_guilds()]
#[authority()]
#[short_desc("Let me repeat your message")]
#[long_desc("Let me repeat your message but without any mentions")]
#[usage("[sentence]")]
#[no_typing()]
async fn echo(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (msg, args) = match data {
        CommandData::Message { msg, args, .. } => (msg, args),
        CommandData::Interaction { .. } => unreachable!(),
    };

    msg.delete_message(&ctx).await?;
    let mut content = args.rest().to_owned();
    content_safe(&ctx, &mut content, msg.guild_id);
    let builder = MessageBuilder::new().content(content);
    msg.create_message(&ctx, builder).await?;

    Ok(())
}
