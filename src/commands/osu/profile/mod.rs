use std::{borrow::Cow, sync::Arc};

use command_macros::{command, HasName, SlashCommand};
use eyre::{Report, Result};
use rosu_v2::prelude::{GameMode, OsuError};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::GameModeOption,
    core::commands::{prefix::Args, CommandOrigin},
    pagination::ProfilePagination,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        interaction::InteractionCommand,
        matcher, ChannelExt, CowUtils, InteractionCommandExt,
    },
    Context,
};

pub use self::{
    data::{ProfileData, Top100Stats},
    graph::graphs,
    graph::{ProfileGraphFlags, ProfileGraphParams},
};

use super::{get_user, require_link, UserArgs};

mod data;
mod graph;

#[derive(CommandModel, CreateCommand, SlashCommand, HasName)]
#[command(name = "profile")]
/// Display statistics of a user
pub struct Profile<'a> {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
    /// Choose an embed type
    embed: Option<ProfileKind>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(Copy, Clone, CommandOption, CreateOption, Debug, Eq, PartialEq)]
pub enum ProfileKind {
    #[option(name = "Compact", value = "compact")]
    Compact,
    #[option(name = "User statistics", value = "user_stats")]
    UserStats,
    #[option(name = "Top100 statistics", value = "top100_stats")]
    Top100Stats,
    #[option(name = "Top100 mods", value = "top100_mods")]
    Top100Mods,
    #[option(name = "Top100 mappers", value = "top100_mappers")]
    Top100Mappers,
    #[option(name = "Mapper statistics", value = "mapper_stats")]
    MapperStats,
}

impl Default for ProfileKind {
    #[inline]
    fn default() -> Self {
        Self::Compact
    }
}

impl<'m> Profile<'m> {
    fn args(mode: GameModeOption, args: Args<'m>) -> Result<Self, String> {
        let mut name = None;
        let mut discord = None;

        for arg in args.map(|arg| arg.cow_to_ascii_lowercase()) {
            if let Some(id) = matcher::get_mention_user(&arg) {
                discord = Some(id);
            } else {
                name = Some(arg);
            }
        }

        Ok(Self {
            mode: Some(mode),
            name,
            embed: None,
            discord,
        })
    }
}

#[command]
#[desc("Display statistics of a user")]
#[usage("[username]")]
#[examples("badewanne3")]
#[alias("profile")]
#[group(Osu)]
async fn prefix_osu(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Profile::args(GameModeOption::Osu, args) {
        Ok(args) => profile(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display statistics of a mania user")]
#[usage("[username]")]
#[examples("badewanne3")]
#[aliases("profilemania", "maniaprofile", "profilem")]
#[group(Mania)]
async fn prefix_mania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Profile::args(GameModeOption::Mania, args) {
        Ok(args) => profile(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display statistics of a taiko user")]
#[usage("[username]")]
#[examples("badewanne3")]
#[aliases("profiletaiko", "taikoprofile", "profilet")]
#[group(Taiko)]
async fn prefix_taiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Profile::args(GameModeOption::Taiko, args) {
        Ok(args) => profile(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display statistics of a ctb user")]
#[usage("[username]")]
#[examples("badewanne3")]
#[aliases(
    "profilectb",
    "ctbprofile",
    "profilec",
    "profilecatch",
    "catchprofile",
    "catch"
)]
#[group(Catch)]
async fn prefix_ctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Profile::args(GameModeOption::Catch, args) {
        Ok(args) => profile(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_profile(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Profile::from_interaction(command.input_data())?;

    profile(ctx, (&mut command).into(), args).await
}

async fn profile(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Profile<'_>) -> Result<()> {
    let owner = orig.user_id()?;

    let config = match ctx.user_config(owner).await {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to get user config"));
        }
    };

    let mode = args
        .mode
        .map(GameMode::from)
        .or(config.mode)
        .unwrap_or(GameMode::Osu);

    let kind = args.embed.unwrap_or_default();
    let guild = orig.guild_id();

    let name = match username!(ctx, orig, args) {
        Some(name) => name,
        None => match config.into_username() {
            Some(name) => name,
            None => return require_link(&ctx, &orig).await,
        },
    };

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(name.as_str(), mode);

    let mut user = match get_user(&ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
    };

    user.mode = mode;

    // Try to get the user discord id that is linked to the osu!user
    let discord_id_fut = ctx.psql().get_discord_from_osu_id(user.user_id);

    let discord_id = match discord_id_fut.await {
        Ok(user) => guild
            .zip(user)
            .filter(|&(guild, user)| ctx.cache.member(guild, user, |_| ()).is_ok())
            .map(|(_, user)| user),
        Err(err) => {
            warn!(
                "{:?}",
                err.wrap_err("Failed to get discord id from osu user id")
            );

            None
        }
    };

    let profile_data = ProfileData::new(user, discord_id);
    let builder = ProfilePagination::builder(kind, profile_data);

    builder
        .profile_components()
        .start_by_update()
        .defer_components()
        .start(ctx, orig)
        .await
}
