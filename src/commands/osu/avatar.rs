use crate::{
    arguments::Args,
    embeds::{AvatarEmbed, EmbedData},
    util::{constants::OSU_API_ISSUE, ApplicationCommandExt, MessageExt},
    BotResult, CommandData, Context, Name,
};

use rosu_v2::error::OsuError;
use std::sync::Arc;
use twilight_model::application::{
    command::{BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption},
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

#[command]
#[short_desc("Display someone's osu! profile picture")]
#[aliases("pfp")]
#[usage("[username]")]
#[example("Badewanne3")]
async fn avatar(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let arg_res = args.next().map(|arg| Args::try_link_name(&ctx, arg));

            let name = match arg_res.transpose() {
                Ok(name) => name,
                Err(content) => return msg.error(&ctx, content).await,
            };

            _avatar(ctx, CommandData::Message { msg, args, num }, name).await
        }
        CommandData::Interaction { command } => slash_avatar(ctx, command).await,
    }
}

async fn _avatar(ctx: Arc<Context>, data: CommandData<'_>, name: Option<Name>) -> BotResult<()> {
    let author_id = data.author()?.id;

    let name = match name.or_else(|| ctx.get_link(author_id.0)) {
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
                    Ok(id) => match ctx.get_link(id) {
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

    _avatar(ctx, command.into(), username).await
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
