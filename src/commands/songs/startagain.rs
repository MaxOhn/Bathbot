use crate::{BotResult, CommandData, Context};

use std::sync::Arc;

#[command]
#[short_desc("https://youtu.be/g7VNvg_QTMw&t=29")]
#[bucket("songs")]
async fn startagain(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (lyrics, delay) = _startagain();

    super::song_send(lyrics, delay, ctx, data).await
}

pub fn _startagain() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "I'm not always perfect, but I'm always myself.",
        "If you don't think I'm worth it - find someone eeeelse.",
        "I won't say I'm sorry, for being who I aaaaaam.",
        "Is the eeeend a chance to start agaaaaain?",
    ];

    (lyrics, 5500)
}
