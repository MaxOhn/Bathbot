use crate::{
    embeds::{CacheEmbed, EmbedData},
    util::MessageExt,
    Args, BotResult, Context,
};

use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("Display stats about the internal cache")]
#[owner()]
async fn cache(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    let stats = ctx.cache.stats();

    let embed = &[CacheEmbed::new(stats, ctx.stats.start_time)
        .into_builder()
        .build()];

    msg.build_response(&ctx, |m| m.embeds(embed)).await?;

    Ok(())
}
