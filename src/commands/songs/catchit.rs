use std::sync::Arc;

use command_macros::command;

use crate::{core::Context, BotResult};

#[command]
#[desc("https://youtu.be/BjFWk0ncr70?t=12")]
#[group(Songs)]
#[flags(SKIP_DEFER)]
async fn prefix_catchit(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let (lyrics, delay) = catchit_();

    super::song(lyrics, delay, ctx, msg.into()).await
}

pub fn catchit_() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "This song is one you won't forget",
        "It will get stuck -- in your head",
        "If it does, then you can't blame me",
        "Just like I said - too catchy",
    ];

    (lyrics, 2500)
}
