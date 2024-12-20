use std::mem;

use bathbot_model::rosu_v2::user::{MedalCompactRkyv, User};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    MessageBuilder,
};
use eyre::{Report, Result};
use rkyv::{
    rancor::{Panic, ResultExt},
    with::{Map, With},
};
use rosu_v2::{model::GameMode, prelude::OsuError, request::UserId};

use super::{H, W};
use crate::{
    commands::osu::{medals::stats as medals_stats, user_not_found},
    core::{commands::CommandOrigin, Context},
    manager::redis::{osu::UserArgs, RedisData},
};

pub async fn medals_graph(
    orig: &CommandOrigin<'_>,
    user_id: UserId,
) -> Result<Option<(RedisData<User>, Vec<u8>)>> {
    let user_args = UserArgs::rosu_id(&user_id, GameMode::Osu).await;

    let mut user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = user_not_found(user_id).await;
            orig.error(content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
    };

    let mut medals = match user {
        RedisData::Original(ref mut user) => mem::take(&mut user.medals),
        RedisData::Archive(ref user) => rkyv::api::deserialize_using::<_, _, Panic>(
            With::<_, Map<MedalCompactRkyv>>::cast(&user.medals),
            &mut (),
        )
        .always_ok(),
    };

    medals.sort_unstable_by_key(|medal| medal.achieved_at);

    let bytes = match medals_stats::graph(&medals, W, H) {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content = format!("`{}` does not have any medals", user.username());
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
