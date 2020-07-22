use super::super::command_issue;
use crate::{
    embeds::{AboutEmbed, EmbedData},
    util::MessageExt,
    BotResult, Context,
};

use std::sync::Arc;
use twilight::builders::embed::EmbedBuilder;
use twilight::model::channel::Message;

#[command]
#[short_desc("Displaying some information about this bot")]
#[aliases("info")]
async fn about(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let data = match AboutEmbed::new(&ctx).await {
        Ok(data) => data,
        Err(why) => {
            msg.respond(&ctx, command_issue("about")).await?;
            return Err(why);
        }
    };
    let eb = data.build(EmbedBuilder::new());
    msg.build_response(&ctx, |m| Ok(m.embed(eb.build())?))
        .await?;
    Ok(())
}
