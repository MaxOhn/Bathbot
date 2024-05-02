use bathbot_macros::command;
use eyre::Result;

#[command]
#[desc("https://youtu.be/BjFWk0ncr70?t=12")]
#[group(Songs)]
#[flags(SKIP_DEFER)]
async fn prefix_catchit(msg: &Message) -> Result<()> {
    let (lyrics, delay) = catchit_();

    super::song(lyrics, delay, msg.into()).await
}

pub fn catchit_() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "This song is one you won't forget",
        "It will get stuck -- in your head",
        "If it does, then you can't blame me",
        "Just like I said - too catchy",
    ];

    (lyrics, 2500)
}
