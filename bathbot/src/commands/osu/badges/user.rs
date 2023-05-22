use std::{collections::BTreeMap, sync::Arc};

use bathbot_util::{
    constants::{GENERAL_ISSUE, OSEKAI_ISSUE, OSU_API_ISSUE},
    MessageBuilder,
};
use eyre::{Report, Result};
use rkyv::{Deserialize, Infallible};
use rosu_v2::{prelude::OsuError, request::UserId};

use super::BadgesUser;
use crate::{
    active::{impls::BadgesPagination, ActiveMessages},
    commands::osu::{require_link, user_not_found},
    core::{commands::CommandOrigin, Context},
    manager::redis::{osu::UserArgs, RedisData},
    util::osu::get_combined_thumbnail,
};

pub(super) async fn user(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: BadgesUser,
) -> Result<()> {
    let owner = orig.user_id()?;

    let user_id = match user_id!(ctx, orig, args) {
        Some(user_id) => user_id,
        None => match ctx.user_config().osu_id(owner).await {
            Ok(Some(id)) => UserId::Id(id),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err.wrap_err("failed to get user id"));
            }
        },
    };

    let user_args_fut = UserArgs::rosu_id(&ctx, &user_id);
    let badges_fut = ctx.redis().badges();

    let (user_args_res, badges_res) = tokio::join!(user_args_fut, badges_fut);

    let (user_id_raw, user_id) = match user_args_res {
        UserArgs::Args(args) => (args.user_id, user_id),
        UserArgs::User { user, .. } => (user.user_id, UserId::Name(user.username)),
        UserArgs::Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        UserArgs::Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user");

            return Err(err);
        }
    };

    let badges = match badges_res {
        Ok(badges) => badges,
        Err(err) => {
            let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get badges"));
        }
    };

    let mut badges = match badges {
        RedisData::Original(mut badges) => {
            badges.retain(|badge| badge.users.contains(&user_id_raw));

            badges
        }
        RedisData::Archive(badges) => badges
            .iter()
            .filter(|badge| badge.users.contains(&user_id_raw))
            .map(|badge| badge.deserialize(&mut Infallible).unwrap())
            .collect(),
    };

    args.sort.unwrap_or_default().apply(&mut badges);

    let owners = if let Some(badge) = badges.first() {
        let owners_fut = ctx.client().get_osekai_badge_owners(badge.badge_id);

        match owners_fut.await {
            Ok(owners) => owners,
            Err(err) => {
                let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

                return Err(err.wrap_err("failed to get badge owners"));
            }
        }
    } else {
        let user_id = match user_id {
            UserId::Id(user_id) => match ctx.osu_user().name(user_id).await {
                Ok(Some(name)) => UserId::Name(name),
                Ok(None) => UserId::Id(user_id),
                Err(err) => {
                    warn!("{err:?}");

                    UserId::Id(user_id)
                }
            },
            user_id @ UserId::Name(_) => user_id,
        };

        let content = match user_id {
            UserId::Id(user_id) => format!("User with id {user_id} has no badges :("),
            UserId::Name(name) => format!("User `{name}` has no badges :("),
        };

        let builder = MessageBuilder::new().embed(content);
        orig.create_message(&ctx, &builder).await?;

        return Ok(());
    };

    let urls = owners.iter().map(|owner| owner.avatar_url.as_ref());

    let bytes = if badges.len() == 1 {
        match get_combined_thumbnail(&ctx, urls, owners.len() as u32, Some(1024)).await {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                warn!(?err, "Failed to combine avatars");

                None
            }
        }
    } else {
        None
    };

    let mut owners_map = BTreeMap::new();
    owners_map.insert(0, owners.into_boxed_slice());

    let pagination = BadgesPagination::builder()
        .badges(badges.into_boxed_slice())
        .owners(owners_map)
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .attachment(bytes.map(|bytes| ("badge_owners.png".to_owned(), bytes)))
        .begin(ctx, orig)
        .await
}
