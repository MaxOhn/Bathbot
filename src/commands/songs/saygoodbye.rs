use super::song_send;
use crate::{Args, BotResult, Context};

use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("https://youtu.be/SyJMQg3spck?t=43")]
#[bucket("songs")]
pub async fn saygoodbye(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
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
    song_send(lyrics, 2500, ctx, msg).await
}
