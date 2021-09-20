use crate::{BotResult, CommandData, Context};

use std::sync::Arc;

#[command]
#[short_desc("https://youtu.be/BjFWk0ncr70?t=12")]
#[bucket("songs")]
#[no_typing()]
async fn catchit(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (lyrics, delay) = _catchit();

    super::song_send(lyrics, delay, ctx, data).await
}

pub fn _catchit() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "This song is one you won't forget",
        "It will get stuck -- in your head",
        "If it does, then you can't blame me",
        "Just like I said - too catchy",
    ];

    (lyrics, 2500)
}
