use crate::{BotResult, Context};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Start the bg game / Skip the current background")]
#[aliases("s", "resolve", "r", "skip")]
pub async fn start(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    Ok(())
}
