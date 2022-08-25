use std::sync::Arc;

use time::Duration;

use crate::{
    util::{builder::MessageBuilder, interaction::InteractionCommand, InteractionCommandExt},
    BotResult, Context,
};

pub async fn trackinginterval(
    ctx: Arc<Context>,
    command: InteractionCommand,
    seconds: i64,
) -> BotResult<()> {
    let interval = Duration::seconds(seconds);
    let previous = ctx.tracking().interval().whole_seconds();
    ctx.tracking().set_interval(interval);

    let content = format!(
        "Tracking interval: {previous}s -> {}s",
        interval.whole_seconds()
    );

    let builder = MessageBuilder::new().embed(content);
    command.callback(&ctx, builder, false).await?;

    Ok(())
}
