use bathbot_macros::command;
use bathbot_model::rosu_v2::user::MedalCompactRkyv;
use bathbot_util::{constants::GENERAL_ISSUE, matcher, MessageBuilder};
use eyre::{Report, Result};
use rkyv::{
    rancor::{Panic, ResultExt},
    with::{Map, With},
};
use rosu_v2::{model::GameMode, prelude::OsuError, request::UserId};
use twilight_model::guild::Permissions;

use super::{Graph, GraphMedals, H, W};
use crate::{
    commands::osu::{graphs::GRAPH_MEDALS_DESC, medals::stats as medals_stats, user_not_found},
    core::{
        commands::{prefix::Args, CommandOrigin},
        Context,
    },
    manager::redis::osu::{CachedUser, UserArgs, UserArgsError},
};

impl<'m> GraphMedals<'m> {
    fn args(args: Args<'m>) -> Self {
        let mut name = None;
        let mut discord = None;

        for arg in args {
            if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Self { name, discord }
    }
}

#[command]
#[desc(GRAPH_MEDALS_DESC)]
#[usage("[username]")]
#[examples("peppy")]
#[group(AllModes)]
async fn prefix_graphmedals(
    msg: &Message,
    args: Args<'_>,
    perms: Option<Permissions>,
) -> Result<()> {
    let args = GraphMedals::args(args);
    let orig = CommandOrigin::from_msg(msg, perms);

    super::graph(orig, Graph::Medals(args)).await
}

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
