use crate::{
    commands::{
        osu::{option_discord, option_name},
        MyCommand,
    },
    database::OsuData,
    embeds::{EmbedData, MostPlayedEmbed},
    pagination::{MostPlayedPagination, Pagination},
    util::{
        constants::{
            common_literals::{DISCORD, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        numbers, ApplicationCommandExt, InteractionExt, MessageExt,
    },
    Args, BotResult, CommandData, Context,
};

use eyre::Report;
use rosu_v2::prelude::{GameMode, OsuError, Username};
use std::sync::Arc;
use twilight_model::application::interaction::{
    application_command::CommandDataOption, ApplicationCommand,
};

#[command]
#[short_desc("Display the most played maps of a user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("mp")]
async fn mostplayed(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let name = match args.next() {
                Some(arg) => match Args::check_user_mention(&ctx, arg).await {
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
    let user_fut = super::request_user(&ctx, &name, GameMode::STD);
    let maps_fut_1 = ctx.osu().user_most_played(name.as_str()).limit(50);
    let maps_fut_2 = ctx
        .osu()
        .user_most_played(name.as_str())
        .limit(50)
        .offset(50);

    let (user, maps) = match tokio::try_join!(user_fut, maps_fut_1, maps_fut_2) {
        Ok((user, mut maps, mut maps_2)) => {
            maps.append(&mut maps_2);

            (user, maps)
        }
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

const MOSTPLAYED: &str = "mostplayed";

async fn parse_username(
    ctx: &Context,
    command: &mut ApplicationCommand,
) -> BotResult<Result<Option<Username>, String>> {
    let mut username = None;

    for option in command.yoink_options() {
        match option {
            CommandDataOption::String { name, value } => match name.as_str() {
                NAME => username = Some(value.into()),
                DISCORD => {
                    username = Some(parse_discord_option!(ctx, value, "mostplayed").into_username())
                }
                _ => bail_cmd_option!(MOSTPLAYED, string, name),
            },
            CommandDataOption::Integer { name, .. } => {
                bail_cmd_option!(MOSTPLAYED, integer, name)
            }
            CommandDataOption::Boolean { name, .. } => {
                bail_cmd_option!(MOSTPLAYED, boolean, name)
            }
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!(MOSTPLAYED, subcommand, name)
            }
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

    MyCommand::new(MOSTPLAYED, "Display the most played maps of a user")
        .options(vec![name, discord])
}
