use crate::{BotResult, CommandData, Context};

use std::sync::Arc;

#[command]
#[short_desc("https://youtu.be/la9C0n7jSsI")]
#[bucket("songs")]
async fn flamingo(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (lyrics, delay) = _flamingo();

    super::song_send(lyrics, delay, ctx, data).await
}

pub fn _flamingo() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "How many shrimps do you have to eat",
        "before you make your skin turn pink?",
        "Eat too much and you'll get sick",
        "Shrimps are pretty rich",
    ];

    (lyrics, 2500)
}
