mod bombsaway;
mod catchit;
mod chicago;
mod ding;
mod fireandflames;
mod fireflies;
mod flamingo;
mod mylove;
mod padoru;
mod pretender;
mod rockefeller;
mod saygoodbye;
mod startagain;
mod tijdmachine;
mod time_traveler;
mod wordsneversaid;
mod zenzenzense;

use std::{fmt::Write, sync::Arc};

use command_macros::SlashCommand;
use eyre::Result;
use tokio::time::{interval, Duration};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};

use crate::{
    core::{buckets::BucketName, commands::CommandOrigin},
    util::{
        builder::MessageBuilder, interaction::InteractionCommand, InteractionCommandExt, MessageExt,
    },
    Context,
};

pub use self::{
    bombsaway::*, catchit::*, chicago::*, ding::*, fireandflames::*, fireflies::*, flamingo::*,
    mylove::*, padoru::*, pretender::*, rockefeller::*, saygoodbye::*, startagain::*,
    tijdmachine::*, time_traveler::*, wordsneversaid::*, zenzenzense::*,
};

async fn song(
    lyrics: &[&str],
    delay: u64,
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
) -> Result<()> {
    debug_assert!(lyrics.len() > 1);

    let (id, allow) = match orig.guild_id() {
        Some(guild) => (guild.get(), ctx.guild_with_lyrics(guild).await),
        None => (orig.user_id()?.get(), true),
    };

    let cooldown = ctx.buckets.get(BucketName::Songs).lock().take(id); // same bucket for guilds

    if cooldown > 0 {
        let content = format!("Command on cooldown, try again in {cooldown} seconds");

        return orig.error(&ctx, content).await;
    }

    if allow {
        let mut interval = interval(Duration::from_millis(delay));
        let len: usize = lyrics.iter().map(|line| line.len()).sum();
        let mut content = String::with_capacity(len + lyrics.len() * 5);

        let _ = writeln!(content, "♫ {} ♫", lyrics[0]);
        let builder = MessageBuilder::new().content(&content);
        interval.tick().await;

        let mut response = orig
            .callback_with_response(&ctx, builder)
            .await?
            .model()
            .await?;

        for line in &lyrics[1..] {
            interval.tick().await;
            let _ = writeln!(content, "♫ {line} ♫");

            let builder = MessageBuilder::new().content(&content);
            response = response.update(&ctx, &builder).await?.model().await?;
        }
    } else {
        let content = "The server's big boys disabled song commands. \
            Server authorities can re-enable them with the `/serverconfig` command";

        orig.error(&ctx, content).await?;
    }

    Ok(())
}

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "song")]
#[flags(SKIP_DEFER)]
/// Let me sing a song for you
pub struct Song {
    #[command(help = "Currently available: \
    [Bombs away](https://youtu.be/xpkkakkDhN4?t=65), \
    [Catchit](https://youtu.be/BjFWk0ncr70?t=12), \
    [Chicago](https://www.youtube.com/watch?v=MWserASk0Jg&t=60s), \
    [Ding](https://youtu.be/_yWU0lFghxU?t=54), \
    [Fireflies](https://youtu.be/psuRGfAaju4?t=25), \
    [Flamingo](https://youtu.be/la9C0n7jSsI), \
    [My Love](https://www.youtube.com/watch?v=V3OPDTwH9os&t=53s), \
    [Padoru](https://youtu.be/u3kRzdSnsTA), \
    [Pretender](https://youtu.be/SBjQ9tuuTJQ?t=83), \
    [Rockefeller Street](https://youtu.be/hjGZLnja1o8?t=41), \
    [Say Goodbye](https://youtu.be/SyJMQg3spck?t=43), \
    [Start Again](https://youtu.be/g7VNvg_QTMw&t=29), \
    [Tijdmachine](https://youtu.be/DT6tpUbWOms?t=47), \
    [Time Traveler](https://youtu.be/iNdDRQFdrmY?t=78), \
    [The words I never said](https://youtu.be/8er4CQCxPRQ?t=65s), \
    [Through the Fire and Flames](https://youtu.be/0jgrCKhxE1s?t=77), \
    [Zen Zen Zense](https://www.youtube.com/watch?v=607QsB38hn8&t=71s)")]
    /// Choose a song title
    title: SongTitle,
}

#[derive(CommandOption, CreateOption)]
pub enum SongTitle {
    #[option(name = "Bombs away", value = "bombsaway")]
    Bombsaway,
    #[option(name = "Catchit", value = "catchit")]
    Catchit,
    #[option(name = "Chicago", value = "chicago")]
    Chicago,
    #[option(name = "Ding", value = "ding")]
    Ding,
    #[option(name = "Fireflies", value = "fireflies")]
    Fireflies,
    #[option(name = "Flamingo", value = "flamingo")]
    Flamingo,
    #[option(name = "My Love", value = "mylove")]
    MyLove,
    #[option(name = "Padoru", value = "padoru")]
    Padoru,
    #[option(name = "Pretender", value = "pretender")]
    Pretender,
    #[option(name = "Rockefeller Street", value = "rockefeller")]
    Rockefeller,
    #[option(name = "Say Goodbye", value = "saygoodbye")]
    SayGoodbye,
    #[option(name = "Start Again", value = "startagain")]
    StartAgain,
    #[option(name = "Tijdmachine", value = "tijdmachine")]
    Tijdmachine,
    #[option(name = "Time Traveler", value = "time_traveler")]
    TimeTraveler,
    #[option(name = "The words I never said", value = "wordsneversaid")]
    WordsNeverSaid,
    #[option(name = "Through the Fire and Flames", value = "fireandflames")]
    FireAndFlames,
    #[option(name = "Zen Zen Zense", value = "zenzenzense")]
    ZenZenZense,
}

impl SongTitle {
    fn get(self) -> (&'static [&'static str], u64) {
        match self {
            Self::Bombsaway => bombsaway_(),
            Self::Catchit => catchit_(),
            Self::Chicago => chicago_(),
            Self::Ding => ding_(),
            Self::Fireflies => fireflies_(),
            Self::Flamingo => flamingo_(),
            Self::MyLove => mylove_(),
            Self::Padoru => padoru_(),
            Self::Pretender => pretender_(),
            Self::Rockefeller => rockefeller_(),
            Self::SayGoodbye => saygoodbye_(),
            Self::StartAgain => startagain_(),
            Self::Tijdmachine => tijdmachine_(),
            Self::TimeTraveler => time_traveler_(),
            Self::WordsNeverSaid => wordsneversaid_(),
            Self::FireAndFlames => fireandflames_(),
            Self::ZenZenZense => zenzenzense_(),
        }
    }
}

pub async fn slash_song(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Song::from_interaction(command.input_data())?;
    let (lyrics, delay) = args.title.get();

    song(lyrics, delay, ctx, (&mut command).into()).await
}
