use crate::{BotResult, CommandData, Context};

use std::sync::Arc;

#[command]
#[short_desc("https://youtu.be/_yWU0lFghxU?t=54")]
#[bucket("songs")]
async fn ding(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (lyrics, delay) = _ding();

    super::song_send(lyrics, delay, ctx, data).await
}

pub fn _ding() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "Oh-oh-oh, hübsches Ding",
        "Ich versteck' mein' Ehering",
        "Klinglingeling, wir könnten's bring'n",
        "Doch wir nuckeln nur am Drink",
        "Oh-oh-oh, hübsches Ding",
        "Du bist Queen und ich bin King",
        "Wenn ich dich seh', dann muss ich sing'n:",
        "Tingalingaling, you pretty thing!",
    ];

    (lyrics, 2500)
}
