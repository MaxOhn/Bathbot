use std::sync::Arc;

use command_macros::command;
use eyre::Result;

use crate::core::Context;

#[command]
#[desc("https://youtu.be/_yWU0lFghxU?t=54")]
#[group(Songs)]
#[flags(SKIP_DEFER)]
async fn prefix_ding(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    let (lyrics, delay) = ding_();

    super::song(lyrics, delay, ctx, msg.into()).await
}

pub fn ding_() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "Oh-oh-oh, hübsches Ding",
        "Ich versteck' mein' Ehering",
        "Klinglingeling, wir könnten's bring'n",
        "Doch wir nuckeln nur am Drink",
        "Oh-oh-oh, hübsches Ding",
        "Du bist Queen und ich bin King",
        "Wenn ich dich seh', dann muss ich sing'n",
        "Tingalingaling, you pretty thing!",
    ];

    (lyrics, 2500)
}
