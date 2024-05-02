use bathbot_macros::command;
use eyre::Result;

#[command]
#[desc("https://youtu.be/la9C0n7jSsI")]
#[group(Songs)]
#[flags(SKIP_DEFER)]
async fn prefix_flamingo(msg: &Message) -> Result<()> {
    let (lyrics, delay) = flamingo_();

    super::song(lyrics, delay, msg.into()).await
}

pub fn flamingo_() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "How many shrimps do you have to eat",
        "before you make your skin turn pink?",
        "Eat too much and you'll get sick",
        "Shrimps are pretty rich",
    ];

    (lyrics, 2500)
}
