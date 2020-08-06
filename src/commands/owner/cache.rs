use crate::{
    embeds::{CacheEmbed, EmbedData},
    util::MessageExt,
    Args, BotResult, Context,
};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Display stats about the internal cache")]
#[owner()]
async fn cache(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    let embed = CacheEmbed::new(&ctx).build().build();
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}
