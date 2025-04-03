use std::collections::{BTreeMap, HashMap};

use crate::{
    active::{ActiveMessages, impls::relax::top::RelaxTopPagination},
    commands::{osu::require_link, owner},
    core::{Context, commands::CommandOrigin},
    manager::redis::osu::{UserArgs, UserArgsError},
};
use bathbot_model::RelaxScore;
use bathbot_util::{EmbedBuilder, MessageBuilder, constants::GENERAL_ISSUE};
use eyre::{Error, Result};
use rosu_v2::{error::OsuError, model::GameMode, request::UserId};

use super::RelaxTop;

pub async fn relax_top(orig: CommandOrigin<'_>, args: RelaxTop<'_>) -> Result<()> {
    top(orig, args).await
}

struct RelaxTopArgs {
    name: Option<String>,
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
            let err = eyre::Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };
    let user_id = user.user_id.to_native();

    let client = Context::client();
    let scores = client.get_relax_player_scores(user_id);
    let scores = scores.await.map(|scores| {
        scores
            .into_iter()
            .enumerate()
            .collect::<BTreeMap<usize, RelaxScore>>()
    });
    let scores = scores?;

    let map_ids = scores
        .values()
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

    let content = String::new().into_boxed_str();

    let pagination = RelaxTopPagination::builder()
        .user(user)
        .content(content)
        .total(scores.len())
        .scores(scores)
        .maps(maps)
        .msg_owner(msg_owner)
        .build();
    // let stub = EmbedBuilder::new().title(format!("{}", scores.unwrap_or_default().len()));
    // let stub_message = MessageBuilder::new().embed(stub);
    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}
