use std::sync::Arc;

use command_macros::command;
use eyre::Result;

use crate::Context;

#[command]
#[desc("https://youtu.be/hjGZLnja1o8?t=41")]
#[group(Songs)]
#[alias("1273")]
#[flags(SKIP_DEFER)]
pub async fn prefix_rockefeller(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    let (lyrics, delay) = rockefeller_();

    super::song(lyrics, delay, ctx, msg.into()).await
}

pub fn rockefeller_() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "1 - 2 - 7 - 3",
        "down the Rockefeller street.",
        "Life is marchin' on, do you feel that?",
        "1 - 2 - 7 - 3",
        "down the Rockefeller street.",
        "Everything is more than surreal",
    ];

    (lyrics, 2250)
}
