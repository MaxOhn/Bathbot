use super::song_send;
use crate::{Args, BotResult, Context};

use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("https://youtu.be/SBjQ9tuuTJQ?t=83")]
#[bucket("songs")]
pub async fn pretender(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    let lyrics = &[
        "What if I say I'm not like the others?",
        "What if I say I'm not just another oooone of your plays?",
        "You're the pretender",
        "What if I say that I will never surrender?",
    ];
    song_send(lyrics, 3000, ctx, msg).await
}
