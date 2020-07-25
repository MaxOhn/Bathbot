use crate::{Args, BotResult, Context};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Get a hint for the current background")]
#[aliases("h", "tip")]
pub async fn hint(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    Ok(())
}
