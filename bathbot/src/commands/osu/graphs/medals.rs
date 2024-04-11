use std::{mem, sync::Arc};

use bathbot_model::rosu_v2::user::{MedalCompact as MedalCompactRkyv, User};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    MessageBuilder,
};
use eyre::{Report, Result};
use rkyv::{
    with::{DeserializeWith, Map},
    Infallible,
};
use rosu_v2::{prelude::OsuError, request::UserId};

use super::{H, W};
use crate::{
    commands::osu::{medals::stats as medals_stats, user_not_found},
    core::{commands::CommandOrigin, Context, ContextExt},
    manager::redis::{osu::UserArgs, RedisData},
};

pub async fn medals_graph(
    ctx: Arc<Context>,
    orig: &CommandOrigin<'_>,
    user_id: UserId,
) -> Result<Option<(RedisData<User>, Vec<u8>)>> {
    let user_args = UserArgs::rosu_id(&ctx, &user_id).await;

    let mut user = match ctx.redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;
            orig.error(&ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
    };

    let mut medals = match user {
        RedisData::Original(ref mut user) => mem::take(&mut user.medals),
        RedisData::Archive(ref user) => {
            Map::<MedalCompactRkyv>::deserialize_with(&user.medals, &mut Infallible).unwrap()
        }
    };

    medals.sort_unstable_by_key(|medal| medal.achieved_at);

    let bytes = match medals_stats::graph(&medals, W, H) {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content = format!("`{}` does not have any medals", user.username());
            let builder = MessageBuilder::new().embed(content);
            orig.create_message(&ctx, builder).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;
            warn!(?err, "Failed to create medals graph");

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}
