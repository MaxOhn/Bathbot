use crate::{BotResult, CommandData, Context};

use std::sync::Arc;

#[command]
#[short_desc("https://youtu.be/SyJMQg3spck?t=43")]
#[bucket("songs")]
async fn saygoodbye(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (lyrics, delay) = _saygoodbye();

    super::song_send(lyrics, delay, ctx, data).await
}

pub fn _saygoodbye() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "It still kills meeee",
        "(it - still - kills - me)",
        "That I can't change thiiiings",
        "(that I - can't - change - things)",
        "But I'm still dreaming",
        "I'll rewrite the ending",
        "So you'll take back the lies",
        "Before we say our goodbyes",
        "\\~\\~\\~ say our goodbyyeees \\~\\~\\~",
    ];

    (lyrics, 2500)
}
