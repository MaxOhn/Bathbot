use crate::{BotResult, CommandData, Context};

use std::sync::Arc;

#[command]
#[short_desc("https://youtu.be/hjGZLnja1o8?t=41")]
#[bucket("songs")]
#[aliases("1273")]
#[no_typing()]
pub async fn rockefeller(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (lyrics, delay) = _rockefeller();

    super::song_send(lyrics, delay, ctx, data).await
}

pub fn _rockefeller() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "1 - 2 - 7 - 3",
        "down the Rockefeller street.",
        "Life is marchin' on, do you feel that?",
        "1 - 2 - 7 - 3",
        "down the Rockefeller street.",
        "Everything is more than surreal",
    ];

    (lyrics, 2250)
}
