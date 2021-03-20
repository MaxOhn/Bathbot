use crate::{tracking::OSU_TRACKING_INTERVAL, util::MessageExt, Args, BotResult, Context};

use chrono::Duration;
use std::{str::FromStr, sync::Arc};
use twilight_model::channel::Message;

#[command]
#[short_desc("Adjust the tracking interval (in seconds) - default 3600")]
#[owner()]
async fn trackinginterval(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let interval = match args.next().map(i64::from_str) {
        Some(Ok(value)) => Duration::seconds(value),
        Some(Err(_)) => return msg.error(&ctx, "Expected i64 as first argument").await,
        None => *OSU_TRACKING_INTERVAL,
    };
    let previous = ctx.tracking().interval.read().await.num_seconds();
    *ctx.tracking().interval.write().await = interval;
    let content = format!(
        "Tracking interval: {}s -> {}s",
        previous,
        interval.num_seconds()
    );
    msg.send_response(&ctx, content).await?;
    Ok(())
}
