use std::sync::Arc;

use eyre::Result;
use time::Duration;

use crate::{
    util::{builder::MessageBuilder, interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

pub async fn trackinginterval(
    ctx: Arc<Context>,
    command: InteractionCommand,
    seconds: i64,
) -> Result<()> {
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
