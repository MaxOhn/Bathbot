use std::sync::Arc;

use command_macros::command;
use eyre::Result;

use crate::Context;

#[command]
#[desc("https://youtu.be/psuRGfAaju4?t=25")]
#[group(Songs)]
#[flags(SKIP_DEFER)]
async fn prefix_fireflies(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    let (lyrics, delay) = fireflies_();

    super::song(lyrics, delay, ctx, msg.into()).await
}

pub fn fireflies_() -> (&'static [&'static str], u64) {
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
