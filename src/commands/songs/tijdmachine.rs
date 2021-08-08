use crate::{BotResult, CommandData, Context};

use std::sync::Arc;

#[command]
#[short_desc("https://youtu.be/DT6tpUbWOms?t=47")]
#[bucket("songs")]
async fn tijdmachine(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (lyrics, delay) = _tijdmachine();

    super::song_send(lyrics, delay, ctx, data).await
}

pub fn _tijdmachine() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "Als ik denk aan al die dagen,",
        "dat ik mij zo heb misdragen.",
        "Dan denk ik, - had ik maar een tijdmachine -- tijdmachine",
        "Maar die heb ik niet,",
        "dus zal ik mij gedragen,",
        "en zal ik blijven sparen,",
        "sparen voor een tijjjdmaaachine.",
    ];

    (lyrics, 2500)
}
