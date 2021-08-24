use crate::{
    util::{constants::GENERAL_ISSUE, ApplicationCommandExt, MessageExt},
    BotResult, CommandData, Context, Error, MessageBuilder,
};

use std::sync::Arc;
use twilight_model::application::{
    command::{BaseCommandOptionData, Command, CommandOption},
    interaction::{application_command::CommandDataOption, ApplicationCommand},
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

    let update_fut = ctx.update_config(guild_id, |config| {
        config.with_lyrics = value.unwrap_or_else(|| !config.with_lyrics);
        with_lyrics = config.with_lyrics;
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

pub async fn slash_togglesongs(
    ctx: Arc<Context>,
    mut command: ApplicationCommand,
) -> BotResult<()> {
    let mut available = None;

    for option in command.yoink_options() {
        match option {
            CommandDataOption::String { name, .. } => {
                bail_cmd_option!("togglesongs", string, name)
            }
            CommandDataOption::Integer { name, .. } => {
                bail_cmd_option!("togglesongs", integer, name)
            }
            CommandDataOption::Boolean { name, value } => match name.as_str() {
                "enable" => available = Some(value),
                _ => bail_cmd_option!("togglesongs", boolean, name),
            },
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!("togglesongs", subcommand, name)
            }
        }
    }

    let available = available.ok_or(Error::InvalidCommandOptions)?;

    _togglesongs(ctx, command.into(), Some(available)).await
}

pub fn slash_togglesongs_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "togglesongs".to_owned(),
        default_permission: None,
        description: "Toggle availability of song commands in a server".to_owned(),
        id: None,
        options: vec![CommandOption::Boolean(BaseCommandOptionData {
            description: "Choose whether song commands can be used or not".to_owned(),
            name: "enable".to_owned(),
            required: true,
        })],
    }
}
