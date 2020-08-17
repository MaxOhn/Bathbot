use crate::{
    embeds::{AboutEmbed, EmbedData},
    util::{constants::GENERAL_ISSUE, MessageExt},
    Args, BotResult, Context,
};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Displaying some information about this bot")]
#[aliases("info")]
async fn about(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    let data = match AboutEmbed::new(&ctx).await {
        Ok(data) => data,
        Err(why) => {
            msg.error(&ctx, GENERAL_ISSUE).await?;
            return Err(why);
        }
    };
    let embed = data.build().build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}
