use bathbot_macros::command;
use eyre::Result;

#[command]
#[desc("https://youtu.be/SBjQ9tuuTJQ?t=83")]
#[group(Songs)]
#[flags(SKIP_DEFER)]
async fn prefix_pretender(msg: &Message) -> Result<()> {
    let (lyrics, delay) = pretender_();

    super::song(lyrics, delay, msg.into()).await
}

pub fn pretender_() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "What if I say I'm not like the others?",
        "What if I say I'm not just another oooone of your plays?",
        "You're the pretender",
        "What if I say that I will never surrender?",
    ];

    (lyrics, 3000)
}
