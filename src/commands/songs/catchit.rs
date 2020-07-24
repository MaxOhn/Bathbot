use super::song_send;
use crate::{BotResult, Context};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("https://youtu.be/BjFWk0ncr70?t=12")]
#[bucket("songs")]
pub async fn catchit(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let lyrics = &[
        "This song is one you won't forget",
        "It will get stuck -- in your head",
        "If it does, then you can't blame me",
        "Just like I said - too catchy",
    ];
    song_send(lyrics, 3000, ctx, msg).await
}
