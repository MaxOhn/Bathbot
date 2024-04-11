use std::sync::Arc;

use bathbot_model::rosu_v2::user::User;
use bathbot_util::{
    constants::{GENERAL_ISSUE, HUISMETBENEN_ISSUE, OSU_API_ISSUE},
    MessageBuilder,
};
use eyre::{Report, Result};
use rosu_v2::{prelude::OsuError, request::UserId};
use time::{Duration, OffsetDateTime};

use super::{H, W};
use crate::{
    commands::osu::{sniped, user_not_found},
    core::{commands::CommandOrigin, Context, ContextExt},
    manager::redis::{osu::UserArgs, RedisData},
};

pub async fn sniped_graph(
    ctx: Arc<Context>,
    orig: &CommandOrigin<'_>,
    user_id: UserId,
) -> Result<Option<(RedisData<User>, Vec<u8>)>> {
    let user_args = UserArgs::rosu_id(ctx.cloned(), &user_id).await;

    let user = match ctx.redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;
            orig.error(&ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user");

            return Err(err);
        }
    };

    let (country_code, username, user_id) = match &user {
        RedisData::Original(user) => {
            let country_code = user.country_code.as_str();
            let username = user.username.as_str();
            let user_id = user.user_id;

            (country_code, username, user_id)
        }
        RedisData::Archive(user) => {
            let country_code = user.country_code.as_str();
            let username = user.username.as_str();
            let user_id = user.user_id;

            (country_code, username, user_id)
        }
    };

    let (sniper, snipee) = if ctx.huismetbenen().is_supported(country_code).await {
        let now = OffsetDateTime::now_utc();
        let sniper_fut =
            ctx.client()
                .get_national_snipes(user_id, true, now - Duration::weeks(8), now);
        let snipee_fut =
            ctx.client()
                .get_national_snipes(user_id, false, now - Duration::weeks(8), now);

        match tokio::try_join!(sniper_fut, snipee_fut) {
            Ok((mut sniper, snipee)) => {
                sniper.retain(|score| score.sniped.is_some());

                (sniper, snipee)
            }
            Err(err) => {
                let _ = orig.error(&ctx, HUISMETBENEN_ISSUE).await;

                return Err(err.wrap_err("failed to get sniper or snipee"));
            }
        }
    } else {
        let content = format!("`{username}`'s country {country_code} is not supported :(");
        orig.error(&ctx, content).await?;

        return Ok(None);
    };

    let bytes = match sniped::graphs(username, &sniper, &snipee, W, H) {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content = format!(
                "`{username}` was neither sniped nor sniped other people in the last 8 weeks"
            );
            let builder = MessageBuilder::new().embed(content);
            orig.create_message(&ctx, builder).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;
            warn!(?err, "Failed to create sniped graph");

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}
