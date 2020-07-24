use crate::{BotResult, Context};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Increase the size of the image")]
#[aliases("b", "enhance")]
pub async fn bigger(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    Ok(())
}
