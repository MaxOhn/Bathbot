use crate::{
    tracking::OSU_TRACKING_INTERVAL, util::MessageExt, BotResult, CommandData, Context,
    MessageBuilder,
};

use chrono::Duration;
use std::{str::FromStr, sync::Arc};

#[command]
#[short_desc("Adjust the tracking interval (in seconds) - default 3600")]
#[owner()]
async fn trackinginterval(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (msg, mut args) = match data {
        CommandData::Message { msg, args, .. } => (msg, args),
        CommandData::Interaction { .. } => unreachable!(),
    };

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

    let builder = MessageBuilder::new().embed(content);
    msg.create_message(&ctx, builder).await?;

    Ok(())
}
