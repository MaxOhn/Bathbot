use bathbot_util::{constants::GENERAL_ISSUE, MessageBuilder};
use eyre::{Report, Result};
use rosu_v2::{model::GameMode, prelude::OsuError, request::UserId};

use super::{H, W};
use crate::{
    commands::osu::{player_snipe_stats, user_not_found},
    core::{commands::CommandOrigin, Context},
    manager::redis::osu::{CachedUser, UserArgs, UserArgsError},
};

pub async fn snipe_count_graph(
    orig: &CommandOrigin<'_>,
    user_id: UserId,
    mode: GameMode,
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

    let country_code = user.country_code.as_str();
    let username = user.username.as_str();
    let user_id = user.user_id.to_native();

    let (player, history) = if Context::huismetbenen()
        .is_supported(country_code, mode)
        .await
    {
        let client = Context::client();
        let player_fut = client.get_snipe_player(country_code, user_id, mode);
        let history_fut = client.get_snipe_player_history(country_code, user_id, mode);

        match tokio::try_join!(player_fut, history_fut) {
            Ok((Some(player), history)) => (player, history),
            Ok((None, _)) => {
                let content = format!(
                    "`{username}` has never had any national #1s in {mode}",
                    mode = match mode {
                        GameMode::Osu => "osu!standard",
                        GameMode::Taiko => "osu!taiko",
                        GameMode::Catch => "osu!catch",
                        GameMode::Mania => "osu!mania",
                    }
                );

                let builder = MessageBuilder::new().embed(content);
                orig.create_message(builder).await?;

                return Ok(None);
            }
            Err(err) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(err);
            }
        }
    } else {
        let content = format!("`{username}`'s country {country_code} is not supported :(");

        orig.error(content).await?;

        return Ok(None);
    };

    let graph_result = player_snipe_stats::graphs(&history, &player.count_sr_spread, W, H);

    let bytes = match graph_result {
        Ok(graph) => graph,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            warn!(?err, "Failed to create snipe count graph");

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}
