use crate::{
    active::impls::commands::osu::require_link,
    core::{commands::CommandOrigin, Context},
    manager::redis::osu::{UserArgs, UserArgsError},
};
use bathbot_util::{constants::GENERAL_ISSUE, EmbedBuilder, MessageBuilder};
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
    let scores = scores.await;

    let pagination = RelaxTopPagination::builder();
    let stub = EmbedBuilder::new().title(format!("{}", scores.unwrap_or_default().len()));
    let stub_message = MessageBuilder::new().embed(stub);
    orig.create_message(stub_message).await?;

    Ok(())
}
