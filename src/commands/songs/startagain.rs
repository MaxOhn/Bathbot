use std::sync::Arc;

use command_macros::command;
use eyre::Result;

use crate::Context;

#[command]
#[desc("https://youtu.be/g7VNvg_QTMw&t=29")]
#[group(Songs)]
#[flags(SKIP_DEFER)]
async fn prefix_startagain(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    let (lyrics, delay) = startagain_();

    super::song(lyrics, delay, ctx, msg.into()).await
}

pub fn startagain_() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "I'm not always perfect, but I'm always myself.",
        "If you don't think I'm worth it - find someone eeeelse.",
        "I won't say I'm sorry, for being who I aaaaaam.",
        "Is the eeeend a chance to start agaaaaain?",
    ];

    (lyrics, 5500)
}
