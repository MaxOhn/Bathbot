use crate::{BotResult, CommandData, Context};

use std::sync::Arc;

#[command]
#[short_desc("https://youtu.be/0jgrCKhxE1s?t=77")]
#[bucket("songs")]
#[no_typing()]
async fn fireandflames(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (lyrics, delay) = _fireandflames();

    super::song_send(lyrics, delay, ctx, data).await
}

pub fn _fireandflames() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "So far away we wait for the day-yay",
        "For the lives all so wasted and gooone",
        "We feel the pain of a lifetime lost in a thousand days",
        "Through the fire and the flames we carry ooooooon",
    ];

    (lyrics, 3000)
}
