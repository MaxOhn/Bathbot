use crate::{
    commands::{MyCommand, MyCommandOption},
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, CommandData, Context, Error, MessageBuilder,
};

use std::sync::Arc;
use twilight_model::application::interaction::{
    application_command::CommandOptionValue, ApplicationCommand,
};

#[command]
#[only_guilds()]
#[authority()]
#[short_desc("Toggle availability of song commands in a server")]
#[long_desc(
    "Toggle whether song commands can be used in this server. \
    Defaults to `true`"
)]
#[aliases("songstoggle", "songtoggle")]
async fn togglesongs(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    _togglesongs(ctx, data, None).await
}

async fn _togglesongs(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    value: Option<bool>,
) -> BotResult<()> {
    let guild_id = data.guild_id().unwrap();
    let mut with_lyrics = false;

    let update_fut = ctx.update_guild_config(guild_id, |config| {
        config.with_lyrics = if value.is_some() {
            value
        } else {
            Some(!config.with_lyrics())
        };

        with_lyrics = config.with_lyrics();
    });

    if let Err(why) = update_fut.await {
        let _ = data.error(&ctx, GENERAL_ISSUE).await;

        return Err(why);
    }

    let content = if with_lyrics {
        "Song commands can now be used in this server"
    } else {
        "Song commands can no longer be used in this server"
    };

    let builder = MessageBuilder::new().embed(content);
    data.create_message(&ctx, builder).await?;

    Ok(())
}

pub async fn slash_togglesongs(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    let option = command.data.options.first().and_then(|option| {
        (option.name == "enable").then(|| match option.value {
            CommandOptionValue::Boolean(value) => Some(value),
            _ => None,
        })
    });

    let available = match option.flatten() {
        Some(value) => value,
        None => return Err(Error::InvalidCommandOptions),
    };

    _togglesongs(ctx, command.into(), Some(available)).await
}

pub fn define_togglesongs() -> MyCommand {
    let enable =
        MyCommandOption::builder("enable", "Choose whether song commands can be used or not")
            .boolean(true);

    let description = "Toggle availability of song commands in a server";

    MyCommand::new("togglesongs", description)
        .options(vec![enable])
        .authority()
}
