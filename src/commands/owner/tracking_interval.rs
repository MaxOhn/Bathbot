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
    match data {
        CommandData::Message { msg, mut args, num } => {
            let seconds = match args.next().map(i64::from_str) {
                Some(Ok(value)) => value,
                Some(Err(_)) => return msg.error(&ctx, "Expected i64 as first argument").await,
                None => OSU_TRACKING_INTERVAL.num_seconds(),
            };

            _trackinginterval(ctx, CommandData::Message { msg, args, num }, seconds).await
        }
        CommandData::Interaction { command } => super::slash_owner(ctx, command).await,
    }
}

pub(super) async fn _trackinginterval(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    seconds: i64,
) -> BotResult<()> {
    let interval = Duration::seconds(seconds);
    let previous = ctx.tracking().interval.read().await.num_seconds();
    *ctx.tracking().interval.write().await = interval;

    let content = format!(
        "Tracking interval: {}s -> {}s",
        previous,
        interval.num_seconds()
    );

    let builder = MessageBuilder::new().embed(content);
    data.create_message(&ctx, builder).await?;

    Ok(())
}
