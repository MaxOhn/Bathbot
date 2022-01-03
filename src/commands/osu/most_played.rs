use std::sync::Arc;

use eyre::Report;
use rosu_v2::prelude::{GameMode, OsuError, Username};
use twilight_model::application::interaction::{
    application_command::CommandOptionValue, ApplicationCommand,
};

use crate::{
    commands::{
        check_user_mention,
        osu::{get_user_cached, option_discord, option_name},
        parse_discord, DoubleResultCow, MyCommand,
    },
    database::OsuData,
    embeds::{EmbedData, MostPlayedEmbed},
    error::Error,
    pagination::{MostPlayedPagination, Pagination},
    util::{
        constants::{
            common_literals::{DISCORD, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        numbers, ApplicationCommandExt, InteractionExt, MessageExt,
    },
    BotResult, CommandData, Context,
};

use super::UserArgs;

#[command]
#[short_desc("Display the most played maps of a user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("mp")]
async fn mostplayed(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
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

            _mostplayed(ctx, CommandData::Message { msg, args, num }, name).await
        }
        CommandData::Interaction { command } => slash_mostplayed(ctx, *command).await,
    }
}

async fn _mostplayed(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    name: Option<Username>,
) -> BotResult<()> {
    let name = match name {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    // Retrieve the user and their most played maps
    let mut user_args = UserArgs::new(name.as_str(), GameMode::STD);

    let result = if let Some(alt_name) = user_args.whitespaced_name() {
        match get_user_cached(&ctx, &user_args).await {
            Ok(user) => ctx
                .osu()
                .user_most_played(user_args.name)
                .limit(100)
                .await
                .map(|maps| (user, maps)),
            Err(OsuError::NotFound) => {
                user_args.name = &alt_name;

                let user_fut = get_user_cached(&ctx, &user_args);
                let maps_fut = ctx.osu().user_most_played(user_args.name).limit(100);

                tokio::try_join!(user_fut, maps_fut)
            }
            Err(err) => Err(err),
        }
    } else {
        let user_fut = get_user_cached(&ctx, &user_args);
        let maps_fut = ctx.osu().user_most_played(user_args.name).limit(100);

        tokio::try_join!(user_fut, maps_fut)
    };

    let (user, maps) = match result {
        Ok((user, maps)) => (user, maps),
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Accumulate all necessary data
    let pages = numbers::div_euclid(10, maps.len());
    let embed_data = MostPlayedEmbed::new(&user, maps.iter().take(10), (1, pages));

    // Creating the embed
    let builder = embed_data.into_builder().build().into();
    let response_raw = data.create_message(&ctx, builder).await?;

    // Skip pagination if too few entries
    if maps.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = MostPlayedPagination::new(response, user, maps);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

async fn parse_username(
    ctx: &Context,
    command: &mut ApplicationCommand,
) -> DoubleResultCow<Option<Username>> {
    let mut username = None;

    for option in command.yoink_options() {
        match option.value {
            CommandOptionValue::String(value) => match option.name.as_str() {
                NAME => username = Some(value.into()),
                _ => return Err(Error::InvalidCommandOptions),
            },
            CommandOptionValue::User(value) => match option.name.as_str() {
                DISCORD => match parse_discord(ctx, value).await? {
                    Ok(osu) => username = Some(osu.into_username()),
                    Err(content) => return Ok(Err(content)),
                },
                _ => return Err(Error::InvalidCommandOptions),
            },
            _ => return Err(Error::InvalidCommandOptions),
        }
    }

    let name = match username {
        Some(name) => Some(name),
        None => ctx
            .psql()
            .get_user_osu(command.user_id()?)
            .await?
            .map(OsuData::into_username),
    };

    Ok(Ok(name))
}

pub async fn slash_mostplayed(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match parse_username(&ctx, &mut command).await {
        Ok(Ok(name)) => _mostplayed(ctx, command.into(), name).await,
        Ok(Err(content)) => command.error(&ctx, content).await,
        Err(why) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            Err(why)
        }
    }
}

pub fn define_mostplayed() -> MyCommand {
    let name = option_name();
    let discord = option_discord();

    MyCommand::new("mostplayed", "Display the most played maps of a user")
        .options(vec![name, discord])
}
