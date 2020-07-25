use super::song_send;
use crate::{Args, BotResult, Context};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("https://youtu.be/DT6tpUbWOms?t=47")]
#[bucket("songs")]
pub async fn tijdmachine(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    let lyrics = &[
        "Als ik denk aan al die dagen,",
        "dat ik mij zo heb misdragen.",
        "Dan denk ik, - had ik maar een tijdmachine -- tijdmachine",
        "Maar die heb ik niet,",
        "dus zal ik mij gedragen,",
        "en zal ik blijven sparen,",
        "sparen voor een tijjjdmaaachine.",
    ];
    song_send(lyrics, 2500, ctx, msg).await
}
