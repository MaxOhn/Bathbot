use bathbot_macros::command;
use eyre::Result;

#[command]
#[desc("https://youtu.be/psuRGfAaju4?t=25")]
#[group(Songs)]
#[flags(SKIP_DEFER)]
async fn prefix_fireflies(msg: &Message) -> Result<()> {
    let (lyrics, delay) = fireflies_();

    super::song(lyrics, delay, msg.into()).await
}

pub fn fireflies_() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "You would not believe your eyes",
        "If ten million fireflies",
        "Lit up the world as I fell asleep",
        "'Cause they'd fill the open air",
        "And leave teardrops everywhere",
        "You'd think me rude, but I would just stand and -- stare",
    ];

    (lyrics, 2500)
}
