use super::song_send;
use crate::{BotResult, Context};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("https://youtu.be/_yWU0lFghxU?t=54")]
#[bucket("songs")]
pub async fn ding(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
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
    song_send(lyrics, 2500, ctx, msg).await
}
