use bathbot_macros::command;
use eyre::Result;

#[command]
#[desc("https://youtu.be/xpkkakkDhN4?t=65")]
#[group(Songs)]
#[flags(SKIP_DEFER)]
pub async fn prefix_bombsaway(msg: &Message) -> Result<()> {
    let (lyrics, delay) = bombsaway_();

    super::song(lyrics, delay, msg.into()).await
}

pub fn bombsaway_() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "Tick tick tock and it's bombs awayyyy",
        "Come ooon, it's the only way",
        "Save your-self for a better dayyyy",
        "No, no, we are falling dooo-ooo-ooo-ooown",
        "I know, you know - this is over",
        "Tick tick tock and it's bombs awayyyy",
        "Now we're falling -- now we're falling doooown",
    ];

    (lyrics, 2750)
}
