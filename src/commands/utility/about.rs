use crate::{BotResult, Context};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Various info about me")]
#[long_desc("Various info about me.")]
#[aliases("info")]
async fn about(_ctx: Arc<Context>, _msg: &Message) -> BotResult<()> {
    Ok(())
}
