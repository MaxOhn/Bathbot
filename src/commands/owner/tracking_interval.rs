use std::sync::Arc;

use chrono::Duration;
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    util::{builder::MessageBuilder, ApplicationCommandExt},
    BotResult, Context,
};

pub async fn trackinginterval(
    ctx: Arc<Context>,
    command: Box<ApplicationCommand>,
    seconds: i64,
) -> BotResult<()> {
    let interval = Duration::seconds(seconds);
    let previous = ctx.tracking().interval.read().num_seconds();
    *ctx.tracking().interval.write() = interval;

    let content = format!(
        "Tracking interval: {previous}s -> {}s",
        interval.num_seconds()
    );

    let builder = MessageBuilder::new().embed(content);
    command.callback(&ctx, builder, false).await?;

    Ok(())
}
