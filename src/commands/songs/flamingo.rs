use super::song_send;
use crate::{BotResult, Context};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("https://youtu.be/la9C0n7jSsI")]
#[bucket("songs")]
pub async fn flamingo(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let lyrics = &[
        "How many shrimps do you have to eat",
        "before you make your skin turn pink?",
        "Eat too much and you'll get sick",
        "Shrimps are pretty rich",
    ];
    song_send(lyrics, 2500, ctx, msg).await
}
