use crate::{BotResult, Context};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Stop the bg game")]
#[aliases("end", "quit")]
pub async fn stop(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    Ok(())
}
