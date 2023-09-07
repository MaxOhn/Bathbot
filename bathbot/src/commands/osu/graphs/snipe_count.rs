use bathbot_model::rosu_v2::user::User;
use bathbot_util::{
    constants::{GENERAL_ISSUE, HUISMETBENEN_ISSUE, OSU_API_ISSUE},
    MessageBuilder,
};
use eyre::{Report, Result};
use rosu_v2::{prelude::OsuError, request::UserId};

use super::{H, W};
use crate::{
    commands::osu::{player_snipe_stats, user_not_found},
    core::{commands::CommandOrigin, Context},
    manager::redis::{osu::UserArgs, RedisData},
};

pub async fn snipe_count_graph(
    ctx: &Context,
    orig: &CommandOrigin<'_>,
    user_id: UserId,
) -> Result<Option<(RedisData<User>, Vec<u8>)>> {
    let user_args = UserArgs::rosu_id(ctx, &user_id).await;

    let user = match ctx.redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = user_not_found(ctx, user_id).await;
            orig.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(ctx, OSU_API_ISSUE).await;
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

    let player = if ctx.huismetbenen().is_supported(country_code).await {
        let player_fut = ctx.client().get_snipe_player(country_code, user_id);

        match player_fut.await {
            Ok(Some(player)) => player,
            Ok(None) => {
                let content = format!("`{username}` has never had any national #1s");
                let builder = MessageBuilder::new().embed(content);
                orig.create_message(ctx, builder).await?;

                return Ok(None);
            }
            Err(err) => {
                let _ = orig.error(ctx, HUISMETBENEN_ISSUE).await;

                return Err(err);
            }
        }
    } else {
        let content = format!("`{username}`'s country {country_code} is not supported :(");

        orig.error(ctx, content).await?;

        return Ok(None);
    };

    let graph_result =
        player_snipe_stats::graphs(&player.count_first_history, &player.count_sr_spread, W, H);

    let bytes = match graph_result {
        Ok(graph) => graph,
        Err(err) => {
            let _ = orig.error(ctx, GENERAL_ISSUE).await;
            warn!(?err, "Failed to create snipe count graph");

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}
