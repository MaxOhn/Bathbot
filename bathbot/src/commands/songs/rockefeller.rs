use bathbot_macros::command;
use eyre::Result;

#[command]
#[desc("https://youtu.be/hjGZLnja1o8?t=41")]
#[group(Songs)]
#[alias("1273")]
#[flags(SKIP_DEFER)]
pub async fn prefix_rockefeller(msg: &Message) -> Result<()> {
    let (lyrics, delay) = rockefeller_();

    super::song(lyrics, delay, msg.into()).await
}

pub fn rockefeller_() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "1 - 2 - 7 - 3",
        "down the Rockefeller street.",
        "Life is marchin' on, do you feel that?",
        "1 - 2 - 7 - 3",
        "down the Rockefeller street.",
        "Everything is more than surreal",
    ];

    (lyrics, 2250)
}
