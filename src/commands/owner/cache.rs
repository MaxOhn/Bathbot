use crate::{
    embeds::{CacheEmbed, EmbedData},
    util::MessageExt,
    BotResult, CommandData, Context, MessageBuilder,
};

use std::sync::Arc;

#[command]
#[short_desc("Display stats about the internal cache")]
#[owner()]
pub(super) async fn cache(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let stats = ctx.cache.stats();
    let embed = CacheEmbed::new(stats, ctx.stats.start_time).into_builder();
    let builder = MessageBuilder::new().embed(embed);
    data.create_message(&ctx, builder).await?;

    Ok(())
}
