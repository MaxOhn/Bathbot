use crate::{BotResult, CommandData, Context};

use std::sync::Arc;

#[command]
#[short_desc("https://youtu.be/xpkkakkDhN4?t=65")]
#[bucket("songs")]
pub async fn bombsaway(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (lyrics, delay) = _bombsaway();

    super::song_send(lyrics, 2750, ctx, data).await
}

pub fn _bombsaway() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "Tick tick tock and it's bombs awayyyy",
        "Come ooon, it's the only way",
        "Save your-self for a better dayyyy",
        "No, no, we are falling dooo-ooo-ooo-ooown",
        "I know, you know - this is over",
        "Tick tick tock and it's bombs awayyyy",
        "Now we're falling -- now we're falling doooown",
    ];

    (lyrics, 2750)
}
