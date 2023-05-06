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
    active::{impls::MostPlayedPagination, ActiveMessages},
    core::commands::CommandOrigin,
    manager::redis::osu::UserArgs,
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, Default, HasName, SlashCommand)]
#[command(name = "mostplayed", desc = "Display the most played maps of a user")]
pub struct MostPlayed<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
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

    let user = match ctx.redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let maps_fut = ctx.osu().user_most_played(user.user_id()).limit(100);

    let maps = match maps_fut.await {
        Ok(maps) => maps,
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get maps");

            return Err(err);
        }
    };

    let pagination = MostPlayedPagination::builder()
        .user(user)
        .maps(maps.into_boxed_slice())
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(ctx, orig)
        .await
}
