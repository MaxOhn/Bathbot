use std::{collections::BTreeMap, sync::Arc};

use eyre::Report;
use rkyv::{Deserialize, Infallible};
use rosu_v2::prelude::{GameMode, OsuError};

use crate::{
    commands::osu::UserArgs,
    core::{commands_::CommandData, Context},
    custom_client::OsekaiBadge,
    database::UserConfig,
    embeds::{BadgeEmbed, EmbedData},
    pagination::{BadgePagination, Pagination},
    util::{
        constants::{OSEKAI_ISSUE, OSU_API_ISSUE},
        get_combined_thumbnail, numbers, MessageBuilder, MessageExt,
    },
    BotResult,
};

use super::BadgeOrder;

pub(super) async fn user_(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    config: UserConfig,
    sort_by: BadgeOrder,
) -> BotResult<()> {
    let name = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
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
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

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

            return data.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    let badges = match badges_result {
        Ok(badges) => badges,
        Err(err) => {
            let _ = data.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
        }
    };

    let mut badges: Vec<OsekaiBadge> = badges
        .get()
        .iter()
        .filter(|badge| badge.users.contains(&user.user_id))
        .map(|badge| badge.deserialize(&mut Infallible).unwrap())
        .collect();

    sort_by.apply(&mut badges);

    let owners = if let Some(badge) = badges.first() {
        let owners_fut = ctx.clients.custom.get_osekai_badge_owners(badge.badge_id);

        match owners_fut.await {
            Ok(owners) => owners,
            Err(err) => {
                let _ = data.error(&ctx, OSEKAI_ISSUE).await;

                return Err(err.into());
            }
        }
    } else {
        let content = format!("User `{name}` has no badges \\:(");
        let builder = MessageBuilder::new().embed(content);
        data.create_message(&ctx, builder).await?;

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

    let embed = BadgeEmbed::new(&badges[0], &owners, (1, pages))
        .into_builder()
        .build();

    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = bytes {
        builder = builder.file("badge_owners.png", bytes);
    }

    let response_raw = data.create_message(&ctx, builder).await?;

    if badges.len() == 1 {
        return Ok(());
    }

    let response = response_raw.model().await?;
    let mut owners_map = BTreeMap::new();
    owners_map.insert(0, owners);

    let pagination = BadgePagination::new(response, badges, owners_map, Arc::clone(&ctx));
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}
