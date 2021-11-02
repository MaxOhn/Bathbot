use crate::{BotResult, CommandData, Context, commands::{MyCommand, check_user_mention, parse_discord}, database::OsuData, embeds::{AvatarEmbed, EmbedData}, error::Error, util::{
        constants::{
            common_literals::{DISCORD, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        ApplicationCommandExt, InteractionExt, MessageExt,
    }};

use rosu_v2::prelude::{GameMode, OsuError, Username};
use std::sync::Arc;
use twilight_model::application::interaction::{
    application_command::CommandOptionValue, ApplicationCommand,
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
                Some(arg) => match check_user_mention(&ctx, arg).await {
                    Ok(Ok(osu)) => Some(osu.into_username()),
                    Ok(Err(content)) => return msg.error(&ctx, content).await,
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                },
                None => match ctx.psql().get_user_osu(msg.author.id).await {
                    Ok(osu) => osu.map(OsuData::into_username),
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

async fn _avatar(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    name: Option<Username>,
) -> BotResult<()> {
    let name = match name {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let user = match super::request_user(&ctx, &name, GameMode::STD).await {
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
    let mut osu = None;

    for option in command.yoink_options() {
        match option.value {
            CommandOptionValue::String(value) => match option.name.as_str() {
                NAME => osu = Some(value.into()),
                _ => return Err(Error::InvalidCommandOptions),
            },
            CommandOptionValue::User(value) => match option.name.as_str() {
                DISCORD => match parse_discord(&ctx, value).await? {
                    Ok(osu_) => osu = Some(osu_),
                    Err(content) => return command.error(&ctx, content).await,
                },
                _ => return Err(Error::InvalidCommandOptions),
            },
            _ => return Err(Error::InvalidCommandOptions),
        }
    }

    let name = match osu {
        Some(osu) => Some(osu.into_username()),
        None => ctx
            .psql()
            .get_user_osu(command.user_id()?)
            .await?
            .map(OsuData::into_username),
    };

    _avatar(ctx, command.into(), name).await
}

pub fn define_avatar() -> MyCommand {
    let name = option_name();
    let discord = option_discord();

    MyCommand::new("avatar", "Display someone's osu! profile picture").options(vec![name, discord])
}
