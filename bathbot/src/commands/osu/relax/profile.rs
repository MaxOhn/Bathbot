use std::{borrow::Cow, time::Instant};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_model::RelaxUser;
use bathbot_util::{constants::GENERAL_ISSUE, MessageBuilder, MessageOrigin};
use eyre::{Error, Report, Result};
use rosu_v2::{error::OsuError, model::GameMode, request::UserId};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    guild::Permissions,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{impls::relax, ActiveMessages},
    commands::osu::{require_link, user_not_found},
    core::{commands::CommandOrigin, Context},
    manager::redis::osu::{UserArgs, UserArgsError},
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

use self::relax::RelaxProfile;

#[derive(CommandModel, CreateCommand, SlashCommand, HasName)]
#[command(
    name = "relax_profile",
    desc = "Display your relax profile",
    help = "Display your relax profile info"
)]
pub struct RelaxPlayerProfile<'a> {
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

async fn slash_relaxplayerprofile(mut command: InteractionCommand) -> Result<()> {
    let args = RelaxPlayerProfile::from_interaction(command.input_data())?;

    relax_player_profile((&mut command).into(), args).await
}

pub(super) async fn relax_player_profile(
    orig: CommandOrigin<'_>,
    args: RelaxPlayerProfile<'_>,
) -> Result<()> {
    let owner = orig.user_id()?;
    let config = Context::user_config().with_osu_id(owner).await?;

    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match config.osu {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&orig).await,
        },
    };
    let user_args = UserArgs::rosu_id(&user_id, GameMode::Osu).await;

    let user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = match user_id {
                UserId::Id(user_id) => format!("User with id {user_id} was not found"),
                UserId::Name(name) => format!("User `{name}` was not found"),
            };

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let client = Context::client();
    let (user_id, no_user_specified) = match user_id!(orig, args) {
        Some(user_id) => (user_id, false),
        None => match config.osu {
            Some(user_id) => (UserId::Id(user_id), true),
            None => return require_link(&orig).await,
        },
    };
    let tz = no_user_specified.then_some(config.timezone).flatten();

    let user_id = user.user_id.to_native();
    let guild = orig.guild_id();

    let user_id_fut = Context::user_config().discord_from_osu_id(user_id);
    let user_id_res = user_id_fut.await;
    // Try to get the discord user id that is linked to the osu!user
    let discord_id = match user_id_res {
        Ok(user) => match (guild, user) {
            (Some(guild), Some(user)) => Context::cache()
                .member(guild, user) // make sure the user is in the guild
                .await?
                .map(|_| user),
            _ => None,
        },
        Err(err) => {
            warn!(?err, "Failed to get discord id from osu user id");

            None
        }
    };
    let info = client.get_relax_player(user_id).await?;

    let origin = MessageOrigin::new(orig.guild_id(), orig.channel_id());
    let owner = orig.user_id()?;
    let mut pagination = relax::RelaxProfile::new(user, discord_id, tz, info, origin, owner);

    let builder = MessageBuilder::new().embed(pagination.compact().unwrap());
    orig.create_message(builder).await?;

    Ok(())
}
