use bathbot_util::constants::GENERAL_ISSUE;
use eyre::{Report, Result};
use rosu_v2::{error::OsuError, model::GameMode, request::UserId};

use super::GraphOsuTrack;
use crate::{
    commands::osu::user_not_found,
    core::{Context, commands::CommandOrigin},
    manager::redis::osu::{CachedUser, UserArgs, UserArgsError},
};

mod accuracy;
mod grades;
mod hit_ratios;
mod playcount;
mod pp_rank;
mod score;

pub async fn osutrack_graph(
    orig: &CommandOrigin<'_>,
    user_id: UserId,
    mode: GameMode,
    args: GraphOsuTrack,
) -> Result<Option<(CachedUser, Vec<u8>)>> {
    let user_args = UserArgs::rosu_id(&user_id, mode).await;

    let user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;
            orig.error(content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let user_id = user.user_id.to_native();

    let history = match Context::redis().osutrack_history(user_id, mode).await {
        Ok(history) if history.is_empty() => {
            let content = format!(
                "`{name}` has no osutrack history :(",
                name = user.username.as_str()
            );

            orig.error(content).await?;

            return Ok(None);
        }
        Ok(history) => history,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get osutrack history");

            return Err(err);
        }
    };

    let res = match args {
        GraphOsuTrack::PpRank(_) => pp_rank::graph(&history),
        GraphOsuTrack::Score(_) => score::graph(&history),
        GraphOsuTrack::HitRatios(_) => hit_ratios::graph(mode, &history),
        GraphOsuTrack::Playcount(_) => playcount::graph(&history),
        GraphOsuTrack::Accuracy(_) => accuracy::graph(&history),
        GraphOsuTrack::Grades(_) => grades::graph(&history),
    };

    Ok(Some((user, res?)))
}
