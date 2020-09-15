use super::song_send;
use crate::{Args, BotResult, Context};

use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("https://youtu.be/xpkkakkDhN4?t=65")]
#[bucket("songs")]
pub async fn bombsaway(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    let lyrics = &[
        "Tick tick tock and it's bombs awayyyy",
        "Come ooon, it's the only way",
        "Save your-self for a better dayyyy",
        "No, no, we are falling dooo-ooo-ooo-ooown",
        "I know, you know - this is over",
        "Tick tick tock and it's bombs awayyyy",
        "Now we're falling -- now we're falling doooown",
    ];
    song_send(lyrics, 2750, ctx, msg).await
}
