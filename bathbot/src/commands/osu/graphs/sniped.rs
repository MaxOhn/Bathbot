use bathbot_macros::command;
use bathbot_util::{constants::GENERAL_ISSUE, matcher, MessageBuilder};
use eyre::{Report, Result};
use rosu_v2::{model::GameMode, prelude::OsuError, request::UserId};
use twilight_model::guild::Permissions;

use super::{Graph, GraphSniped, H, W};
use crate::{
    commands::osu::{graphs::GRAPH_SNIPED_DESC, sniped, user_not_found, SnipeGameMode},
    core::{
        commands::{prefix::Args, CommandOrigin},
        Context,
    },
    manager::redis::osu::{CachedUser, UserArgs, UserArgsError},
};

impl<'m> GraphSniped<'m> {
    fn args(mode: Option<SnipeGameMode>, args: Args<'m>) -> Self {
        let mut name = None;
        let mut discord = None;

        for arg in args {
            if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Self {
            mode,
            name,
            discord,
        }
    }
}

#[command]
#[desc(GRAPH_SNIPED_DESC)]
#[usage("[username]")]
#[examples("peppy")]
#[group(Osu)]
async fn prefix_graphsniped(
    msg: &Message,
    args: Args<'_>,
    perms: Option<Permissions>,
) -> Result<()> {
    let args = GraphSniped::args(None, args);
    let orig = CommandOrigin::from_msg(msg, perms);

    super::graph(orig, Graph::Sniped(args)).await
}

#[command]
#[desc(GRAPH_SNIPED_DESC)]
#[usage("[username]")]
#[examples("peppy")]
#[aliases("graphsnipedcatch")]
#[group(Catch)]
async fn prefix_graphsnipedctb(
    msg: &Message,
    args: Args<'_>,
    perms: Option<Permissions>,
) -> Result<()> {
    let args = GraphSniped::args(Some(SnipeGameMode::Catch), args);
    let orig = CommandOrigin::from_msg(msg, perms);

    super::graph(orig, Graph::Sniped(args)).await
}

#[command]
#[desc(GRAPH_SNIPED_DESC)]
#[usage("[username]")]
#[examples("peppy")]
#[group(Mania)]
async fn prefix_graphsnipedmania(
    msg: &Message,
    args: Args<'_>,
    perms: Option<Permissions>,
) -> Result<()> {
    let args = GraphSniped::args(Some(SnipeGameMode::Mania), args);
    let orig = CommandOrigin::from_msg(msg, perms);

    super::graph(orig, Graph::Sniped(args)).await
}

pub async fn sniped_graph(
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

    let (mut sniper, mut snipee) = if Context::huismetbenen()
        .is_supported(country_code, mode)
        .await
    {
        let client = Context::client();

        let sniper_fut = client.get_sniped_players(user_id, true, mode);
        let snipee_fut = client.get_sniped_players(user_id, false, mode);

        match tokio::try_join!(sniper_fut, snipee_fut) {
            Ok(tuple) => tuple,
            Err(err) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(err.wrap_err("failed to get sniper or snipee"));
            }
        }
    } else {
        let content = format!("`{username}`'s country {country_code} is not supported :(");
        orig.error(content).await?;

        return Ok(None);
    };

    let bytes = match sniped::graphs(username, &mut sniper, &mut snipee, W, H) {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content = format!(
                "`{username}` neither sniped others nor was sniped by others in the last 8 weeks"
            );
            let builder = MessageBuilder::new().embed(content);
            orig.create_message(builder).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            warn!(?err, "Failed to create sniped graph");

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}
