mod bombsaway;
mod catchit;
mod ding;
mod fireandflames;
mod fireflies;
mod flamingo;
mod pretender;
mod rockefeller;
mod saygoodbye;
mod startagain;
mod tijdmachine;
mod wordsneversaid;

pub use bombsaway::*;
pub use catchit::*;
pub use ding::*;
pub use fireandflames::*;
pub use fireflies::*;
pub use flamingo::*;
pub use pretender::*;
pub use rockefeller::*;
pub use saygoodbye::*;
pub use startagain::*;
pub use tijdmachine::*;
pub use wordsneversaid::*;

use crate::{util::MessageExt, BotResult, CommandData, Context, Error, MessageBuilder};

use std::{fmt::Write, sync::Arc};
use tokio::time::{interval, Duration};
use twilight_model::application::{
    command::CommandOptionChoice,
    interaction::{application_command::CommandOptionValue, ApplicationCommand},
};

use super::{MyCommand, MyCommandOption};

async fn song_send(
    lyrics: &[&str],
    delay: u64,
    ctx: Arc<Context>,
    data: CommandData<'_>,
) -> BotResult<()> {
    debug_assert!(lyrics.len() > 1);

    let allow = match data.guild_id() {
        Some(id) => ctx.config_lyrics(id).await,
        None => true,
    };

    if allow {
        let mut interval = interval(Duration::from_millis(delay));
        let len: usize = lyrics.iter().map(|line| line.len()).sum();
        let mut content = String::with_capacity(len + lyrics.len() * 5);

        let _ = writeln!(content, "♫ {} ♫", lyrics[0]);
        let builder = MessageBuilder::new().content(&content);
        interval.tick().await;
        let mut response = data.create_message(&ctx, builder).await?.model().await?;

        for line in &lyrics[1..] {
            interval.tick().await;
            let _ = writeln!(content, "♫ {} ♫", line);

            response = ctx
                .http
                .update_message(response.channel_id, response.id)
                .content(Some(&content))?
                .exec()
                .await?
                .model()
                .await?;
        }
    } else {
        let content = "The server's big boys disabled song commands. \
            Server authorities can re-enable them with the `togglesongs` command";

        data.error(&ctx, content).await?;
    }

    Ok(())
}

pub async fn slash_song(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    let option = command.data.options.first().and_then(|option| {
        (option.name == "title").then(|| match &option.value {
            CommandOptionValue::String(value) => match value.as_str() {
                "bombsaway" => Some(_bombsaway()),
                "catchit" => Some(_catchit()),
                "ding" => Some(_ding()),
                "fireandflames" => Some(_fireandflames()),
                "fireflies" => Some(_fireflies()),
                "flamingo" => Some(_flamingo()),
                "pretender" => Some(_pretender()),
                "rockefeller" => Some(_rockefeller()),
                "saygoodbye" => Some(_saygoodbye()),
                "startagain" => Some(_startagain()),
                "tijdmachine" => Some(_tijdmachine()),
                "wordsneversaid" => Some(_wordsneversaid()),
                _ => None,
            },
            _ => None,
        })
    });

    let (lyrics, delay) = match option.flatten() {
        Some(tuple) => tuple,
        None => return Err(Error::InvalidCommandOptions),
    };

    song_send(lyrics, delay, ctx, command.into()).await
}

pub fn define_song() -> MyCommand {
    let choices = vec![
        CommandOptionChoice::String {
            name: "Bombs away".to_owned(),
            value: "bombsaway".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Catchit".to_owned(),
            value: "catchit".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Ding".to_owned(),
            value: "ding".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Fireflies".to_owned(),
            value: "fireflies".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Flamingo".to_owned(),
            value: "flamingo".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Pretender".to_owned(),
            value: "pretender".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Rockefeller Street".to_owned(),
            value: "rockefeller".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Say Goodbye".to_owned(),
            value: "saygoodbye".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Start Again".to_owned(),
            value: "startagain".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Tijdmachine".to_owned(),
            value: "tijdmachine".to_owned(),
        },
        CommandOptionChoice::String {
            name: "The words I never said".to_owned(),
            value: "wordsneversaid".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Through the Fire and Flames".to_owned(),
            value: "fireandflames".to_owned(),
        },
    ];

    let help = "Currently available: \
        [Bombs away](https://youtu.be/xpkkakkDhN4?t=65), \
        [Catchit](https://youtu.be/BjFWk0ncr70?t=12), \
        [Ding](https://youtu.be/_yWU0lFghxU?t=54), \
        [Fireflies](https://youtu.be/psuRGfAaju4?t=25), \
        [Flamingo](https://youtu.be/la9C0n7jSsI), \
        [Pretender](https://youtu.be/SBjQ9tuuTJQ?t=83), \
        [Rockefeller Street](https://youtu.be/hjGZLnja1o8?t=41), \
        [Say Goodbye](https://youtu.be/SyJMQg3spck?t=43), \
        [Start Again](https://youtu.be/g7VNvg_QTMw&t=29), \
        [Tijdmachine](https://youtu.be/DT6tpUbWOms?t=47), \
        [The words I never said](https://youtu.be/8er4CQCxPRQ?t=65s), \
        [Through the Fire and Flames](https://youtu.be/0jgrCKhxE1s?t=77)";

    let title = MyCommandOption::builder("title", "Choose a song title")
        .help(help)
        .string(choices, true);

    MyCommand::new("song", "Let me sing a song for you").options(vec![title])
}
