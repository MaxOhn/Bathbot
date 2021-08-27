use crate::{
    arguments::Args,
    embeds::{AvatarEmbed, EmbedData},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        ApplicationCommandExt, MessageExt,
    },
    BotResult, CommandData, Context, Name,
};

use rosu_v2::error::OsuError;
use std::sync::Arc;
use twilight_model::{
    application::{
        command::{BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption},
        interaction::{application_command::CommandDataOption, ApplicationCommand},
    },
    id::UserId,
};

#[command]
#[short_desc("Display someone's osu! profile picture")]
#[aliases("pfp")]
#[usage("[username]")]
#[example("Badewanne3")]
async fn avatar(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let name = match args.next() {
                Some(arg) => match Args::check_user_mention(&ctx, arg).await {
                    Ok(Ok(name)) => Some(name),
                    Ok(Err(content)) => return msg.error(&ctx, content).await,
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                },
                None => match ctx.user_config(msg.author.id).await {
                    Ok(config) => config.name,
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                },
            };

            _avatar(ctx, CommandData::Message { msg, args, num }, name).await
        }
        CommandData::Interaction { command } => slash_avatar(ctx, *command).await,
    }
}

async fn _avatar(ctx: Arc<Context>, data: CommandData<'_>, name: Option<Name>) -> BotResult<()> {
    let name = match name {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let user = match super::request_user(&ctx, &name, None).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let builder = AvatarEmbed::new(user).into_builder().build().into();
    data.create_message(&ctx, builder).await?;

    Ok(())
}

pub async fn slash_avatar(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let mut username = None;

    for option in command.yoink_options() {
        match option {
            CommandDataOption::String { name, value } => match name.as_str() {
                "name" => username = Some(value.into()),
                "discord" => match value.parse() {
                    Ok(id) => match ctx.user_config(UserId(id)).await?.name {
                        Some(name) => username = Some(name),
                        None => {
                            let content = format!("<@{}> is not linked to an osu profile", id);
                            command.error(&ctx, content).await?;

                            return Ok(());
                        }
                    },
                    Err(_) => bail_cmd_option!("avatar discord", string, value),
                },
                _ => bail_cmd_option!("avatar", string, name),
            },
            CommandDataOption::Integer { name, .. } => bail_cmd_option!("avatar", integer, name),
            CommandDataOption::Boolean { name, .. } => bail_cmd_option!("avatar", boolean, name),
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!("avatar", subcommand, name)
            }
        }
    }

    let name = match username {
        Some(name) => Some(name),
        None => ctx.user_config(command.user_id()?).await?.name,
    };

    _avatar(ctx, command.into(), name).await
}

pub fn slash_avatar_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "avatar".to_owned(),
        default_permission: None,
        description: "Display someone's osu! profile picture".to_owned(),
        id: None,
        options: vec![
            CommandOption::String(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify a username".to_owned(),
                name: "name".to_owned(),
                required: false,
            }),
            CommandOption::User(BaseCommandOptionData {
                description: "Specify a linked discord user".to_owned(),
                name: "discord".to_owned(),
                required: false,
            }),
        ],
    }
}
