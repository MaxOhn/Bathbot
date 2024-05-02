use bathbot_macros::command;
use eyre::Result;

#[command]
#[desc("https://youtu.be/DT6tpUbWOms?t=47")]
#[group(Songs)]
#[flags(SKIP_DEFER)]
async fn prefix_tijdmachine(msg: &Message) -> Result<()> {
    let (lyrics, delay) = tijdmachine_();

    super::song(lyrics, delay, msg.into()).await
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
