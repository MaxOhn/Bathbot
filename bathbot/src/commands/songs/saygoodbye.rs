use bathbot_macros::command;
use eyre::Result;

#[command]
#[desc("https://youtu.be/SyJMQg3spck?t=43")]
#[group(Songs)]
#[flags(SKIP_DEFER)]
async fn prefix_saygoodbye(msg: &Message) -> Result<()> {
    let (lyrics, delay) = saygoodbye_();

    super::song(lyrics, delay, msg.into()).await
}

pub fn saygoodbye_() -> (&'static [&'static str], u64) {
    let lyrics = &[
        "It still kills meeee",
        "(it - still - kills - me)",
        "That I can't change thiiiings",
        "(that I - can't - change - things)",
        "But I'm still dreaming",
        "I'll rewrite the ending",
        "So you'll take back the lies",
        "Before we say our goodbyes",
        "\\~\\~\\~ say our goodbyyeees \\~\\~\\~",
    ];

    (lyrics, 2500)
}
