use std::{collections::BTreeMap, sync::Arc};

use eyre::Report;
use rkyv::{Deserialize, Infallible};
use rosu_v2::prelude::{GameMode, OsuError};

use crate::{
    commands::osu::{require_link, UserArgs},
    core::{commands::CommandOrigin, Context},
    custom_client::OsekaiBadge,
    embeds::{BadgeEmbed, EmbedData},
    pagination::{BadgePagination, Pagination},
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSEKAI_ISSUE, OSU_API_ISSUE},
        get_combined_thumbnail, numbers,
    },
    BotResult,
};

use super::BadgesUser;

pub(super) async fn user(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: BadgesUser,
) -> BotResult<()> {
    let owner = orig.user_id()?;

    let name = match username!(ctx, orig, args) {
        Some(name) => name,
        None => match ctx.psql().get_user_osu(owner).await {
            Ok(Some(osu)) => osu.into_username(),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let mut user = UserArgs::new(name.as_str(), GameMode::STD);
    let redis = ctx.redis();

    let (user_result, badges_result) = if let Some(alt_name) = user.whitespaced_name() {
        match redis.osu_user(&user).await {
            Ok(u) => (Ok(u), redis.badges().await),
            Err(OsuError::NotFound) => {
                user.name = &alt_name;

                tokio::join!(redis.osu_user(&user), redis.badges())
            }
            Err(err) => {
                let _ = orig.error(&ctx, OSU_API_ISSUE).await;

                return Err(err.into());
            }
        }
    } else {
        tokio::join!(redis.osu_user(&user), redis.badges())
    };

    let user = match user_result {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    let badges = match badges_result {
        Ok(badges) => badges,
        Err(err) => {
            let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
        }
    };

    let mut badges: Vec<OsekaiBadge> = badges
        .get()
        .iter()
        .filter(|badge| badge.users.contains(&user.user_id))
        .map(|badge| badge.deserialize(&mut Infallible).unwrap())
        .collect();

    args.sort.unwrap_or_default().apply(&mut badges);

    let owners = if let Some(badge) = badges.first() {
        let owners_fut = ctx.client().get_osekai_badge_owners(badge.badge_id);

        match owners_fut.await {
            Ok(owners) => owners,
            Err(err) => {
                let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

                return Err(err.into());
            }
        }
    } else {
        let content = format!("User `{name}` has no badges \\:(");
        let builder = MessageBuilder::new().embed(content);
        orig.create_message(&ctx, &builder).await?;

        return Ok(());
    };

    let urls = owners.iter().map(|owner| owner.avatar_url.as_str());

    let bytes = if badges.len() == 1 {
        match get_combined_thumbnail(&ctx, urls, owners.len() as u32, Some(1024)).await {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to combine avatars");
                warn!("{report:?}");

                None
            }
        }
    } else {
        None
    };

    let pages = numbers::div_euclid(1, badges.len());

    let embed = BadgeEmbed::new(&badges[0], &owners, (1, pages));
    let mut builder = MessageBuilder::new().embed(embed.build());

    if let Some(bytes) = bytes {
        builder = builder.attachment("badge_owners.png", bytes);
    }

    let response_raw = orig.create_message(&ctx, &builder).await?;

    if badges.len() == 1 {
        return Ok(());
    }

    let response = response_raw.model().await?;
    let mut owners_map = BTreeMap::new();
    owners_map.insert(0, owners);

    let pagination = BadgePagination::new(response, badges, owners_map, Arc::clone(&ctx));

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}
