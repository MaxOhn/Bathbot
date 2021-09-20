use crate::{BotResult, CommandData, Context};

use std::sync::Arc;

#[command]
#[short_desc("https://youtu.be/SBjQ9tuuTJQ?t=83")]
#[bucket("songs")]
#[no_typing()]
async fn pretender(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (lyrics, delay) = _pretender();

    super::song_send(lyrics, delay, ctx, data).await
}

pub fn _pretender() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "What if I say I'm not like the others?",
        "What if I say I'm not just another oooone of your plays?",
        "You're the pretender",
        "What if I say that I will never surrender?",
    ];

    (lyrics, 3000)
}
