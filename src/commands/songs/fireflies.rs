use super::song_send;
use crate::{BotResult, Context};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("https://youtu.be/psuRGfAaju4?t=25")]
#[bucket("songs")]
pub async fn fireflies(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let lyrics = &[
        "You would not believe your eyes",
        "If ten million fireflies",
        "Lit up the world as I fell asleep",
        "'Cause they'd fill the open air",
        "And leave teardrops everywhere",
        "You'd think me rude, but I would just stand and -- stare",
    ];
    song_send(lyrics, 2500, ctx, msg).await
}
