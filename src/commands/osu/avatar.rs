use crate::{
    arguments::Args,
    commands::MyCommand,
    embeds::{AvatarEmbed, EmbedData},
    util::{
        constants::{
            common_literals::{DISCORD, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        ApplicationCommandExt, InteractionExt, MessageExt,
    },
    BotResult, CommandData, Context, Name,
};

use rosu_v2::error::OsuError;
use std::sync::Arc;
use twilight_model::{
    application::interaction::{application_command::CommandDataOption, ApplicationCommand},
    id::UserId,
};

use super::{option_discord, option_name};

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
                    Ok(config) => config.osu_username,
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

const AVATAR: &str = "avatar";

pub async fn slash_avatar(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let mut username = None;

    for option in command.yoink_options() {
        match option {
            CommandDataOption::String { name, value } => match name.as_str() {
                NAME => username = Some(value.into()),
                DISCORD => match value.parse() {
                    Ok(id) => match ctx.user_config(UserId(id)).await?.osu_username {
                        Some(name) => username = Some(name),
                        None => {
                            let content = format!("<@{}> is not linked to an osu profile", id);
                            command.error(&ctx, content).await?;

                            return Ok(());
                        }
                    },
                    Err(_) => bail_cmd_option!("avatar discord", string, value),
                },
                _ => bail_cmd_option!(AVATAR, string, name),
            },
            CommandDataOption::Integer { name, .. } => bail_cmd_option!(AVATAR, integer, name),
            CommandDataOption::Boolean { name, .. } => bail_cmd_option!(AVATAR, boolean, name),
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!(AVATAR, subcommand, name)
            }
        }
    }

    let name = match username {
        Some(name) => Some(name),
        None => ctx.user_config(command.user_id()?).await?.osu_username,
    };

    _avatar(ctx, command.into(), name).await
}

pub fn define_avatar() -> MyCommand {
    let name = option_name();
    let discord = option_discord();

    MyCommand::new(AVATAR, "Display someone's osu! profile picture").options(vec![name, discord])
}
