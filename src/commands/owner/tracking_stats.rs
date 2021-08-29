use crate::{
    embeds::{EmbedData, TrackingStatsEmbed},
    util::MessageExt,
    BotResult, CommandData, Context,
};

use std::sync::Arc;

#[command]
#[short_desc("Display stats about osu!tracking")]
#[owner()]
pub(super) async fn trackingstats(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let stats = ctx.tracking().stats();
    let builder = TrackingStatsEmbed::new(stats).into_builder().build().into();
    data.create_message(&ctx, builder).await?;

    Ok(())
}
