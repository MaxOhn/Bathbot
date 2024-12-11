/// Try to extract an osu! user from the `args`' fields `name` or `discord`
macro_rules! user_id {
    ($orig:ident, $args:ident) => {
        match crate::commands::osu::HasName::user_id(&$args) {
            crate::commands::osu::UserIdResult::Id(user_id) => Some(user_id),
            crate::commands::osu::UserIdResult::None => None,
            crate::commands::osu::UserIdResult::Future(fut) => match fut.await {
                crate::commands::osu::UserIdFutureResult::Id(user_id) => Some(user_id),
                crate::commands::osu::UserIdFutureResult::NotLinked(user_id) => {
                    let content = format!("<@{user_id}> is not linked to an osu!profile");

                    return $orig.error(content).await;
                }
                crate::commands::osu::UserIdFutureResult::Err(err) => {
                    let content = bathbot_util::constants::GENERAL_ISSUE;
                    let _ = $orig.error(content).await;

                    return Err(err);
                }
            },
        }
    };
}

/// Tries to extract the username and mode from args.
/// If either fails, it checks the user config.
/// If the osu user is still not found, return the linking error.
/// If the mode is still not found, pick GameMode::Osu.
///
/// Only use this when the user config is not needed otherwise,
/// else you'll have to query multiple times from the DB.
macro_rules! user_id_mode {
    ($orig:ident, $args:ident) => {{
        let mode = $args.mode.map(rosu_v2::prelude::GameMode::from);

        if let Some(user_id) = user_id!($orig, $args) {
            if let Some(mode) = mode {
                (user_id, mode)
            } else {
                let mode = crate::core::Context::user_config()
                    .mode($orig.user_id()?)
                    .await?
                    .unwrap_or(rosu_v2::prelude::GameMode::Osu);

                (user_id, mode)
            }
        } else {
            let config = crate::core::Context::user_config()
                .with_osu_id($orig.user_id()?)
                .await?;

            let mode = mode
                .or(config.mode)
                .unwrap_or(rosu_v2::prelude::GameMode::Osu);

            match config.osu {
                Some(user_id) => (rosu_v2::request::UserId::Id(user_id), mode),
                None => return crate::commands::osu::require_link(&$orig).await,
            }
        }
    }};
}

use std::{future::Future, pin::Pin};

use bathbot_util::osu::ModSelection;
use eyre::{Report, Result, WrapErr};
use rosu_v2::request::UserId;
use twilight_interactions::command::{CommandOption, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

pub use self::{
    badges::*, claim_name::*, compare::*, fix::*, graphs::*, leaderboard::*, map::*, map_search::*,
    match_compare::*, match_costs::*, medals::*, nochoke::*, osustats::*, profile::*, recent::*,
    render::*, simulate::*, snipe::*, top::*, whatif::*,
};
use crate::{
    core::commands::{interaction::InteractionCommands, CommandOrigin},
    Context,
};

mod attributes;
mod avatar;
mod badges;
mod bookmarks;
mod bws;
mod cards;
mod claim_name;
mod compare;
mod fix;
mod graphs;
mod leaderboard;
mod map;
mod map_search;
mod mapper;
mod match_compare;
mod match_costs;
mod medals;
mod most_played;
mod nochoke;
mod osekai;
mod osustats;
mod pinned;
mod pp;
mod profile;
mod rank;
mod ranking;
mod ratios;
mod recent;
mod render;
mod serverleaderboard;
mod simulate;
mod snipe;
mod top;
mod whatif;

#[cfg(feature = "server")]
mod link;

#[cfg(feature = "matchlive")]
mod match_live;

pub trait HasMods {
    fn mods(&self) -> ModsResult;
}

pub enum ModsResult {
    Mods(ModSelection),
    None,
    Invalid,
}

pub trait HasName {
    fn user_id(&self) -> UserIdResult;
}

pub enum UserIdResult {
    Id(UserId),
    None,
    Future(Pin<Box<dyn Future<Output = UserIdFutureResult> + Send>>),
}

pub enum UserIdFutureResult {
    Id(UserId),
    NotLinked(Id<UserMarker>),
    Err(Report),
}

pub async fn require_link(orig: &CommandOrigin<'_>) -> Result<()> {
    let link = InteractionCommands::get_command("link").map_or_else(
        || "`/link`".to_owned(),
        |cmd| cmd.mention("link").to_string(),
    );

    let content =
        format!("Either specify an osu! username or link yourself to an osu! profile via {link}");

    orig.error(content)
        .await
        .wrap_err("Failed to send require-link message")
}

pub async fn user_not_found(user_id: UserId) -> String {
    let user_id = match user_id {
        user_id @ UserId::Name(_) => user_id,
        UserId::Id(user_id) => match Context::osu_user().name(user_id).await {
            Ok(Some(name)) => UserId::Name(name),
            Ok(None) => UserId::Id(user_id),
            Err(err) => {
                warn!("{err:?}");

                UserId::Id(user_id)
            }
        },
    };

    match user_id {
        UserId::Name(name) => format!("User `{name}` was not found"),
        UserId::Id(user_id) => format!("User with id {user_id} was not found"),
    }
}

#[derive(Copy, Clone, Eq, PartialEq, CommandOption, CreateOption)]
pub enum ScoreOrder {
    #[option(name = "Accuracy", value = "acc")]
    Acc,
    #[option(name = "BPM", value = "bpm")]
    Bpm,
    #[option(name = "Combo", value = "combo")]
    Combo,
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "Length", value = "len")]
    Length,
    #[option(name = "Misses", value = "misses")]
    Misses,
    #[option(name = "PP", value = "pp")]
    Pp,
    #[option(name = "Map ranked date", value = "ranked_date")]
    RankedDate,
    #[option(name = "Score", value = "score")]
    Score,
    #[option(name = "Stars", value = "stars")]
    Stars,
}

impl Default for ScoreOrder {
    #[inline]
    fn default() -> Self {
        Self::Pp
    }
}

enum UserExtraction {
    Id(UserId),
    Err(Report),
    Content(String),
    None,
}
