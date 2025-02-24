use bathbot_model::rosu_v2::user::MedalCompactRkyv;
use bathbot_util::{MessageBuilder, constants::GENERAL_ISSUE};
use eyre::{Report, Result};
use rkyv::{
    rancor::{Panic, ResultExt},
    with::{Map, With},
};
use rosu_v2::{model::GameMode, prelude::OsuError, request::UserId};

use super::{H, W};
use crate::{
    commands::osu::{medals::stats as medals_stats, user_not_found},
    core::{Context, commands::CommandOrigin},
    manager::redis::osu::{CachedUser, UserArgs, UserArgsError},
};

pub async fn medals_graph(
    orig: &CommandOrigin<'_>,
    user_id: UserId,
) -> Result<Option<(CachedUser, Vec<u8>)>> {
    let user_args = UserArgs::rosu_id(&user_id, GameMode::Osu).await;

    let user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;
            orig.error(content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let report = Report::new(err).wrap_err("Failed to get user");

            return Err(report);
        }
    };

    let mut medals = rkyv::api::deserialize_using::<_, _, Panic>(
        With::<_, Map<MedalCompactRkyv>>::cast(&user.medals),
        &mut (),
    )
    .always_ok();

    medals.sort_unstable_by_key(|medal| medal.achieved_at);

    let bytes = match medals_stats::graph(&medals, W, H) {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content = format!("`{}` does not have any medals", user.username.as_str());
            let builder = MessageBuilder::new().embed(content);
            orig.create_message(builder).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            warn!(?err, "Failed to create medals graph");

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}
