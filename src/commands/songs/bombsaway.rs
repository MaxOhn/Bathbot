use std::sync::Arc;

use command_macros::command;

use crate::{BotResult, Context};

#[command]
#[desc("https://youtu.be/xpkkakkDhN4?t=65")]
#[group(Songs)]
#[flags(SKIP_DEFER)]
pub async fn prefix_bombsaway(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let (lyrics, delay) = bombsaway_();

    super::song(lyrics, delay, ctx, msg.into()).await
}

pub fn bombsaway_() -> (&'static [&'static str], u64) {
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
