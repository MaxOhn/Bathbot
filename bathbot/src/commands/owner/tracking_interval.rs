use bathbot_util::MessageBuilder;
use eyre::Result;
use time::Duration;

use crate::{
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

pub async fn trackinginterval(command: InteractionCommand, seconds: i64) -> Result<()> {
    let tracking = Context::tracking();
    let interval = Duration::seconds(seconds);
    let previous = tracking.interval().whole_seconds();
    tracking.set_interval(interval);

    let content = format!(
        "Tracking interval: {previous}s -> {}s",
        interval.whole_seconds()
    );

    let builder = MessageBuilder::new().embed(content);
    command.callback(builder, false).await?;

    Ok(())
}
