use crate::{util::MessageExt, BotResult, CommandData, Context, MessageBuilder};

use std::sync::{atomic::Ordering, Arc};

#[command]
#[short_desc("Toggle osu!tracking")]
#[owner()]
pub(super) async fn trackingtoggle(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    ctx.tracking()
        .stop_tracking
        .fetch_nand(true, Ordering::SeqCst);

    let current = ctx.tracking().stop_tracking.load(Ordering::Acquire);
    let content = format!("Tracking toggle: {current} -> {}", !current);
    let builder = MessageBuilder::new().embed(content);
    data.create_message(&ctx, builder).await?;

    Ok(())
}
