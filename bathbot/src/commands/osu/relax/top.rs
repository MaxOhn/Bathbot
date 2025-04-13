use std::{cmp::Ordering, collections::HashMap};

use bathbot_util::constants::GENERAL_ISSUE;
use eyre::{Report, Result};
use rosu_v2::{error::OsuError, model::GameMode, request::UserId};

use super::RelaxTop;
use crate::{
    active::{
        ActiveMessages,
        impls::relax::top::{RelaxTopOrder, RelaxTopPagination},
    },
    commands::osu::require_link,
    core::{Context, commands::CommandOrigin},
    manager::redis::osu::{UserArgs, UserArgsError},
};

pub async fn relax_top(orig: CommandOrigin<'_>, args: RelaxTop<'_>) -> Result<()> {
    top(orig, args).await
}

pub async fn top(orig: CommandOrigin<'_>, args: RelaxTop<'_>) -> Result<()> {
    let msg_owner = orig.user_id()?;
    let mut config = match Context::user_config().with_osu_id(msg_owner).await {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match config.osu.take() {
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
    let user_id = user.user_id.to_native();

    let client = Context::client();
    let scores_fut = client.get_relax_player_scores(user_id);
    let player_fut = client.get_relax_player(user_id);

    let relax_api_result = tokio::try_join!(scores_fut, player_fut);

    let (mut scores, player) = match relax_api_result {
        Ok((scores, Some(player))) => (scores, player),
        Ok((_, None)) => {
            return orig
                .error(format!("Relax user `{}` not found", user.username))
                .await;
        }
        Err(e) => {
            let _ = orig.error("Failed to get a relax player").await;

            return Err(e.wrap_err("Failed to get a relax player"));
        }
    };

    scores.retain(|sc| sc.is_best);

    let map_ids = scores
        .iter()
        .take(5)
        .map(|score| (score.beatmap_id as i32, None))
        .collect();

    let maps = match Context::osu_map().maps(&map_ids).await {
        Ok(maps) => maps,
        Err(err) => {
            warn!(?err, "Failed to get maps from database");

            HashMap::default()
        }
    };

    match args.sort.unwrap_or_default() {
        RelaxTopOrder::Acc => scores.sort_unstable_by(|lhs, rhs| {
            rhs.accuracy
                .partial_cmp(&lhs.accuracy)
                .unwrap_or(Ordering::Equal)
        }),
        RelaxTopOrder::Bpm => scores.sort_unstable_by(|lhs, rhs| {
            rhs.beatmap
                .beats_per_minute
                .total_cmp(&lhs.beatmap.beats_per_minute)
        }),
        RelaxTopOrder::Combo => scores.sort_unstable_by(|lhs, rhs| rhs.combo.cmp(&lhs.combo)),
        RelaxTopOrder::Date => scores.sort_unstable_by(|lhs, rhs| rhs.date.cmp(&lhs.date)),
        RelaxTopOrder::Misses => {
            scores.sort_unstable_by(|lhs, rhs| rhs.count_miss.cmp(&lhs.count_miss))
        }
        RelaxTopOrder::ModsCount => {
            scores.sort_unstable_by(|lhs, rhs| rhs.mods.len().cmp(&lhs.mods.len()))
        }
        RelaxTopOrder::Pp => scores
            .sort_unstable_by(|lhs, rhs| rhs.pp.partial_cmp(&lhs.pp).unwrap_or(Ordering::Equal)),
        RelaxTopOrder::Score => {
            scores.sort_unstable_by(|lhs, rhs| rhs.total_score.cmp(&lhs.total_score))
        }
    }

    let pagination = RelaxTopPagination::builder()
        .user(user)
        .relax_user(player)
        .scores(scores)
        .maps(maps)
        .msg_owner(msg_owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}
