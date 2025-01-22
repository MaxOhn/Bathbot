use std::borrow::Cow;

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_model::command_fields::GameModeOption;
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    constants::{GENERAL_ISSUE, },
    matcher, CowUtils, MessageOrigin,
};
use eyre::{Report, Result};
use rosu_v2::{
    prelude::{GameMode, OsuError},
    request::UserId,
};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

use super::{require_link, user_not_found};
use crate::{
    active::{impls::ProfileMenu, ActiveMessages},
    core::commands::{prefix::Args, CommandOrigin},
    manager::redis::osu::{UserArgs, UserArgsError},
    util::{interaction::InteractionCommand, ChannelExt, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, SlashCommand, HasName)]
#[command(name = "profile", desc = "Display statistics of a user")]
pub struct Profile<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(desc = "Choose an embed type")]
    embed: Option<ProfileKind>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
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
async fn prefix_osu(msg: &Message, args: Args<'_>) -> Result<()> {
    match Profile::args(GameModeOption::Osu, args) {
        Ok(args) => profile(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

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
async fn prefix_mania(msg: &Message, args: Args<'_>) -> Result<()> {
    match Profile::args(GameModeOption::Mania, args) {
        Ok(args) => profile(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

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
async fn prefix_taiko(msg: &Message, args: Args<'_>) -> Result<()> {
    match Profile::args(GameModeOption::Taiko, args) {
        Ok(args) => profile(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

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
    "catch",
    "catchthebeat",
    "fruits"
)]
#[group(Catch)]
async fn prefix_ctb(msg: &Message, args: Args<'_>) -> Result<()> {
    match Profile::args(GameModeOption::Catch, args) {
        Ok(args) => profile(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

async fn slash_profile(mut command: InteractionCommand) -> Result<()> {
    let args = Profile::from_interaction(command.input_data())?;

    profile((&mut command).into(), args).await
}

async fn profile(orig: CommandOrigin<'_>, args: Profile<'_>) -> Result<()> {
    let owner = orig.user_id()?;

    let config = match Context::user_config().with_osu_id(owner).await {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to get user config"));
        }
    };

    let mode = args
        .mode
        .map(GameMode::from)
        .or(config.mode)
        .unwrap_or(GameMode::Osu);

    let kind = args.embed.unwrap_or_default();
    let guild = orig.guild_id();

    let legacy_scores = match config.score_data {
        Some(score_data) => score_data.is_legacy(),
        None => match guild {
            Some(guild_id) => Context::guild_config()
                .peek(guild_id, |config| {
                    config.score_data.map(ScoreData::is_legacy)
                })
                .await
                .unwrap_or(false),
            None => false,
        },
    };

    let (user_id, no_user_specified) = match user_id!(orig, args) {
        Some(user_id) => (user_id, false),
        None => match config.osu {
            Some(user_id) => (UserId::Id(user_id), true),
            None => return require_link(&orig).await,
        },
    };

    // Retrieve the user and their top scores
    let user_args = UserArgs::rosu_id(&user_id, mode).await;

    let user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let user_id = user.user_id.to_native();
    let peaks_fut = Context::client().osu_user_rank_acc_peak(user_id, mode);
    let user_id_fut = Context::user_config().discord_from_osu_id(user_id);

    let (peaks_res, user_id_res) = tokio::join!(peaks_fut, user_id_fut);

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

    let peaks = match peaks_res {
        Ok(peaks) => peaks,
        Err(err) => {
            warn!(?err, "Failed to get osutrack peaks");

            None
        }
    };

    let tz = no_user_specified.then_some(config.timezone).flatten();
    let origin = MessageOrigin::new(orig.guild_id(), orig.channel_id());

    let pagination = ProfileMenu::new(
        user,
        discord_id,
        tz,
        peaks,
        legacy_scores,
        kind,
        origin,
        owner,
    );

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}
