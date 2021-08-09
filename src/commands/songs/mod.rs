mod bombsaway;
mod catchit;
mod ding;
mod fireandflames;
mod fireflies;
mod flamingo;
mod pretender;
mod rockefeller;
mod saygoodbye;
mod tijdmachine;

pub use bombsaway::*;
pub use catchit::*;
pub use ding::*;
pub use fireandflames::*;
pub use fireflies::*;
pub use flamingo::*;
pub use pretender::*;
pub use rockefeller::*;
pub use saygoodbye::*;
pub use tijdmachine::*;

use crate::{
    util::{ApplicationCommandExt, MessageExt},
    BotResult, CommandData, Context, Error, MessageBuilder,
};

use std::sync::Arc;
use tokio::time::{interval, Duration};
use twilight_model::application::{
    command::{ChoiceCommandOptionData, Command, CommandOption, CommandOptionChoice},
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

async fn song_send(
    lyrics: &[&str],
    delay: u64,
    ctx: Arc<Context>,
    data: CommandData<'_>,
) -> BotResult<()> {
    let allow = data
        .guild_id()
        .map_or(true, |guild_id| ctx.config_lyrics(guild_id));

    // let channel_id = data.channel_id();

    if allow {
        let mut interval = interval(Duration::from_millis(delay));

        for line in lyrics {
            interval.tick().await;
            let builder = MessageBuilder::new().content(format!("♫ {} ♫", line));

            // TODO: Test
            data.create_message(&ctx, builder).await?;

            // ctx.http
            //     .create_message(channel_id)
            //     .content(&)?
            //     .exec()
            //     .await?;
        }
    } else {
        let content = "The server's big boys disabled song commands. \
            Server authorities can re-enable them with the `togglesongs` command";

        data.error(&ctx, content).await?;
    }

    Ok(())
}

pub async fn slash_song(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let mut song = None;

    for option in command.yoink_options() {
        match option {
            CommandDataOption::String { name, value } => match name.as_str() {
                "title" => {
                    song = match value.as_str() {
                        "bombsaway" => Some(_bombsaway()),
                        "catchit" => Some(_catchit()),
                        "ding" => Some(_ding()),
                        "fireandflames" => Some(_fireandflames()),
                        "fireflies" => Some(_fireflies()),
                        "flamingo" => Some(_flamingo()),
                        "pretender" => Some(_pretender()),
                        "rockefeller" => Some(_rockefeller()),
                        "saygoodbye" => Some(_saygoodbye()),
                        "tijdmachine" => Some(_tijdmachine()),
                        _ => bail_cmd_option!("song title", string, value),
                    };
                }
                _ => bail_cmd_option!("song", string, value),
            },
            CommandDataOption::Integer { name, .. } => bail_cmd_option!("song", integer, name),
            CommandDataOption::Boolean { name, .. } => bail_cmd_option!("song", boolean, name),
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!("song", subcommand, name)
            }
        }
    }

    let (lyrics, delay) = song.ok_or(Error::InvalidCommandOptions)?;

    song_send(lyrics, delay, ctx, command.into()).await
}

pub fn slash_song_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "song".to_owned(),
        default_permission: None,
        description: "Let me sing a song for you".to_owned(),
        id: None,
        options: vec![CommandOption::String(ChoiceCommandOptionData {
            choices: vec![
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
                    name: "Tijdmachine".to_owned(),
                    value: "tijdmachine".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "Through the Fire and Flames".to_owned(),
                    value: "fireandflames".to_owned(),
                },
            ],
            description: "Choose the song title".to_owned(),
            name: "title".to_owned(),
            required: true,
        })],
    }
}
