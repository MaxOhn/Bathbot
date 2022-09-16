use std::sync::Arc;

use command_macros::command;
use eyre::Result;

use crate::Context;

#[command]
#[desc("https://youtu.be/0jgrCKhxE1s?t=77")]
#[group(Songs)]
#[flags(SKIP_DEFER)]
async fn prefix_fireandflames(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    let (lyrics, delay) = fireandflames_();

    super::song(lyrics, delay, ctx, msg.into()).await
}

pub fn fireandflames_() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "So far away we wait for the day-yay",
        "For the lives all so wasted and gooone",
        "We feel the pain of a lifetime lost in a thousand days",
        "Through the fire and the flames we carry ooooooon",
    ];

    (lyrics, 3000)
}
