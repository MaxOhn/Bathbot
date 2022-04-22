use std::sync::Arc;

use command_macros::command;

use crate::{BotResult, Context};

#[command]
#[desc("https://youtu.be/DT6tpUbWOms?t=47")]
#[group(Songs)]
#[flags(SKIP_DEFER)]
async fn prefix_tijdmachine(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let (lyrics, delay) = tijdmachine_();

    super::song(lyrics, delay, ctx, msg.into()).await
}

pub fn tijdmachine_() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "Als ik denk aan al die dagen,",
        "dat ik mij zo heb misdragen.",
        "Dan denk ik, - had ik maar een tijdmachine -- tijdmachine",
        "Maar die heb ik niet,",
        "dus zal ik mij gedragen,",
        "en zal ik blijven sparen,",
        "sparen voor een tiiijdmaaachine.",
    ];

    (lyrics, 2500)
}
