use std::{borrow::Cow, sync::Arc};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
};
use eyre::{Report, Result};
use rosu_v2::{prelude::OsuError, request::UserId};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use super::{require_link, user_not_found};
use crate::{
    core::commands::CommandOrigin,
    manager::redis::osu::UserArgs,
    pagination::MostPlayedPagination,
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

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

async fn slash_mostplayed(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = MostPlayed::from_interaction(command.input_data())?;

    mostplayed(ctx, (&mut command).into(), args).await
}

#[command]
#[desc("Display the most played maps of a user")]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("mp")]
#[group(AllModes)]
async fn prefix_mostplayed(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> Result<()> {
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
) -> Result<()> {
    let owner = orig.user_id()?;

    let user_id = match user_id!(ctx, orig, args) {
        Some(user_id) => user_id,
        None => match ctx.user_config().osu_id(owner).await {
            Ok(Some(user_id)) => UserId::Id(user_id),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    // Retrieve the user and their most played maps
    let user_args = UserArgs::rosu_id(&ctx, &user_id).await;
    let user_fut = ctx.redis().osu_user(user_args);
    let maps_fut = ctx.osu().user_most_played(user_id.clone()).limit(100);

    let (user, maps) = match tokio::try_join!(user_fut, maps_fut) {
        Ok((user, maps)) => (user, maps),
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user or maps");

            return Err(err);
        }
    };

    MostPlayedPagination::builder(user, maps)
        .start_by_update()
        .start(ctx, orig)
        .await
}
