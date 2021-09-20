use crate::{BotResult, CommandData, Context};

use std::sync::Arc;

#[command]
#[short_desc("https://youtu.be/psuRGfAaju4?t=25")]
#[bucket("songs")]
#[no_typing()]
async fn fireflies(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (lyrics, delay) = _fireflies();

    super::song_send(lyrics, delay, ctx, data).await
}

pub fn _fireflies() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "You would not believe your eyes",
        "If ten million fireflies",
        "Lit up the world as I fell asleep",
        "'Cause they'd fill the open air",
        "And leave teardrops everywhere",
        "You'd think me rude, but I would just stand and -- stare",
    ];

    (lyrics, 2500)
}
