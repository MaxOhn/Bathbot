use std::{borrow::Cow, sync::Arc};

use command_macros::{command, HasName, SlashCommand};
use rosu_v2::prelude::{GameMode, OsuError};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::ApplicationCommand,
    id::{marker::UserMarker, Id},
};

use crate::{
    core::commands::CommandOrigin,
    embeds::{EmbedData, MostPlayedEmbed},
    pagination::{MostPlayedPagination, Pagination},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher, numbers, ApplicationCommandExt,
    },
    BotResult, Context,
};

use super::{require_link, UserArgs};

#[derive(CommandModel, CreateCommand, Default, HasName, SlashCommand)]
#[command(name = "mostplayed")]
/// Display the most played maps of a user
pub struct MostPlayed<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

async fn slash_mostplayed(
    ctx: Arc<Context>,
    mut command: Box<ApplicationCommand>,
) -> BotResult<()> {
    let args = MostPlayed::from_interaction(command.input_data())?;

    mostplayed(ctx, command.into(), args).await
}

#[command]
#[desc("Display the most played maps of a user")]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("mp")]
#[group(AllModes)]
async fn prefix_mostplayed(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> BotResult<()> {
    let args = match args.next() {
        Some(arg) => match matcher::get_mention_user(arg) {
            Some(id) => MostPlayed {
                name: None,
                discord: Some(id),
            },
            None => MostPlayed {
                name: Some(Cow::Borrowed(arg)),
                discord: None,
            },
        },
        None => MostPlayed::default(),
    };

    mostplayed(ctx, msg.into(), args).await
}

async fn mostplayed(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: MostPlayed<'_>,
) -> BotResult<()> {
    let owner = orig.user_id()?;

    let name = match username!(ctx, orig, args) {
        Some(name) => name,
        None => match ctx.psql().get_user_osu(owner).await {
            Ok(Some(osu)) => osu.into_username(),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    // Retrieve the user and their most played maps
    let mut user_args = UserArgs::new(name.as_str(), GameMode::STD);

    let result = if let Some(alt_name) = user_args.whitespaced_name() {
        match ctx.redis().osu_user(&user_args).await {
            Ok(user) => ctx
                .osu()
                .user_most_played(user_args.name)
                .limit(100)
                .await
                .map(|maps| (user, maps)),
            Err(OsuError::NotFound) => {
                user_args.name = &alt_name;
                let redis = ctx.redis();

                let user_fut = redis.osu_user(&user_args);
                let maps_fut = ctx.osu().user_most_played(user_args.name).limit(100);

                tokio::try_join!(user_fut, maps_fut)
            }
            Err(err) => Err(err),
        }
    } else {
        let redis = ctx.redis();
        let maps_fut = ctx.osu().user_most_played(user_args.name).limit(100);

        tokio::try_join!(redis.osu_user(&user_args), maps_fut)
    };

    let (user, maps) = match result {
        Ok((user, maps)) => (user, maps),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    // Accumulate all necessary data
    let pages = numbers::div_euclid(10, maps.len());
    let embed_data = MostPlayedEmbed::new(&user, maps.iter().take(10), (1, pages));

    // Creating the embed
    let builder = embed_data.build().into();
    let response_raw = orig.create_message(&ctx, &builder).await?;

    // Skip pagination if too few entries
    if maps.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    MostPlayedPagination::new(response, user, maps).start(ctx, owner, 60);

    Ok(())
}
