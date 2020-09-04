use crate::{tracking::OSU_TRACKING_COOLDOWN, util::MessageExt, Args, BotResult, Context};

use chrono::Duration;
use std::{str::FromStr, sync::Arc};
use twilight::model::channel::Message;

#[command]
#[short_desc("Adjust the tracking cooldown (in ms) - default 5000")]
#[owner()]
async fn trackingcooldown(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let cooldown = match args.next().map(i64::from_str) {
        Some(Ok(value)) => Duration::milliseconds(value),
        Some(Err(_)) => return msg.error(&ctx, "Expected i64 as first argument").await,
        None => *OSU_TRACKING_COOLDOWN,
    };
    let previous = ctx.tracking().cooldown.read().await.num_milliseconds();
    *ctx.tracking().cooldown.write().await = cooldown;
    let content = format!(
        "Tracking cooldown: {}ms -> {}ms",
        previous,
        cooldown.num_milliseconds()
    );
    msg.respond(&ctx, content).await?;
    Ok(())
}
