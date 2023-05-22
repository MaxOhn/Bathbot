/// Try to extract an osu! user from the `args`' fields `name` or `discord`
macro_rules! user_id {
    ($ctx:ident, $orig:ident, $args:ident) => {
        match crate::commands::osu::HasName::user_id(&$args, &$ctx) {
            crate::commands::osu::UserIdResult::Id(user_id) => Some(user_id),
            crate::commands::osu::UserIdResult::None => None,
            crate::commands::osu::UserIdResult::Future(fut) => match fut.await {
                crate::commands::osu::UserIdFutureResult::Id(user_id) => Some(user_id),
                crate::commands::osu::UserIdFutureResult::NotLinked(user_id) => {
                    let content = format!("<@{user_id}> is not linked to an osu!profile");

                    return $orig.error(&$ctx, content).await;
                }
                crate::commands::osu::UserIdFutureResult::Err(err) => {
                    let content = bathbot_util::constants::GENERAL_ISSUE;
                    let _ = $orig.error(&$ctx, content).await;

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
    ($ctx:ident, $orig:ident, $args:ident) => {{
        let mode = $args.mode.map(rosu_v2::prelude::GameMode::from);

        if let Some(user_id) = user_id!($ctx, $orig, $args) {
            if let Some(mode) = mode {
                (user_id, mode)
            } else {
                let mode = $ctx
                    .user_config()
                    .mode($orig.user_id()?)
                    .await?
                    .unwrap_or(rosu_v2::prelude::GameMode::Osu);

                (user_id, mode)
            }
        } else {
            let config = $ctx.user_config().with_osu_id($orig.user_id()?).await?;

            let mode = mode
                .or(config.mode)
                .unwrap_or(rosu_v2::prelude::GameMode::Osu);

            match config.osu {
                Some(user_id) => (rosu_v2::request::UserId::Id(user_id), mode),
                None => return crate::commands::osu::require_link(&$ctx, &$orig).await,
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

#[cfg(feature = "server")]
pub use self::link::*;
#[cfg(feature = "matchlive")]
pub use self::match_live::*;
pub use self::{
    attributes::*, avatar::*, badges::*, bws::*, cards::*, claim_name::*, compare::*,
    country_top::*, fix::*, graphs::*, leaderboard::*, map::*, map_search::*, mapper::*,
    match_compare::*, match_costs::*, medals::*, most_played::*, nochoke::*, osekai::*,
    osustats::*, pinned::*, popular::*, pp::*, profile::*, rank::*, ranking::*, ratios::*,
    recent::*, scores::*, serverleaderboard::*, simulate::*, snipe::*, top::*, whatif::*,
};
use crate::{core::commands::CommandOrigin, Context};

mod attributes;
mod avatar;
mod badges;
mod bws;
mod cards;
mod claim_name;
mod compare;
mod country_top;
mod fix;
mod graphs;
mod leaderboard;
mod link;
mod map;
mod map_search;
mod mapper;
mod match_compare;
mod match_costs;
mod match_live;
mod medals;
mod most_played;
mod nochoke;
mod osekai;
mod osustats;
mod pinned;
mod popular;
mod pp;
mod profile;
mod rank;
mod ranking;
mod ratios;
mod recent;
mod scores;
mod serverleaderboard;
mod simulate;
mod snipe;
mod top;
mod whatif;

pub trait HasMods {
    fn mods(&self) -> ModsResult;
}

pub enum ModsResult {
    Mods(ModSelection),
    None,
    Invalid,
}

pub trait HasName {
    fn user_id<'ctx>(&self, ctx: &'ctx Context) -> UserIdResult<'ctx>;
}

pub enum UserIdResult<'ctx> {
    Id(UserId),
    None,
    Future(Pin<Box<dyn Future<Output = UserIdFutureResult> + 'ctx + Send>>),
}

pub enum UserIdFutureResult {
    Id(UserId),
    NotLinked(Id<UserMarker>),
    Err(Report),
}

pub async fn require_link(ctx: &Context, orig: &CommandOrigin<'_>) -> Result<()> {
    let content = "Either specify an osu! username or link yourself to an osu! profile via `/link`";

    orig.error(ctx, content)
        .await
        .wrap_err("failed to send require-link message")
}

pub async fn user_not_found(ctx: &Context, user_id: UserId) -> String {
    let user_id = match user_id {
        user_id @ UserId::Name(_) => user_id,
        UserId::Id(user_id) => match ctx.osu_user().name(user_id).await {
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
