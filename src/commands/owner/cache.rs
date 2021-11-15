use crate::{
    embeds::{CacheEmbed, EmbedData},
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use std::sync::Arc;

#[command]
#[short_desc("Display stats about the internal cache")]
#[owner()]
pub(super) async fn cache(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let stats = match ctx.cache.stats().await {
        Ok(stats) => stats,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why.into());
        }
    };

    let embed = CacheEmbed::new(stats, ctx.stats.start_time).into_builder();
    let builder = MessageBuilder::new().embed(embed);
    data.create_message(&ctx, builder).await?;

    Ok(())
}
