use super::song_send;
use crate::{Args, BotResult, Context};

use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("https://youtu.be/0jgrCKhxE1s?t=77")]
#[bucket("songs")]
pub async fn fireandflames(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    let lyrics = &[
        "So far away we wait for the day-yay",
        "For the lives all so wasted and gooone",
        "We feel the pain of a lifetime lost in a thousand days",
        "Through the fire and the flames we carry ooooooon",
    ];
    song_send(lyrics, 3000, ctx, msg).await
}
