use bathbot_util::{constants::GENERAL_ISSUE, fields, EmbedBuilder, FooterBuilder, MessageBuilder};
use eyre::{Report, Result};
use rkyv::rancor::{Panic, ResultExt};
use rosu_v2::{error::OsuError, model::GameMode, request::UserId};
use time::OffsetDateTime;

use super::DailyChallengeUser;
use crate::{
    commands::osu::{require_link, user_not_found},
    core::{commands::CommandOrigin, Context},
    manager::redis::osu::{UserArgs, UserArgsError},
    util::{interaction::InteractionCommand, Authored, CachedUserExt, InteractionCommandExt},
};

pub(super) async fn user(mut command: InteractionCommand, user: DailyChallengeUser) -> Result<()> {
    let owner = command.user_id()?;

    let orig = CommandOrigin::Interaction {
        command: &mut command,
    };

    let user_id = match user_id!(orig, user) {
        Some(user_id) => user_id,
        None => match Context::user_config().osu_id(owner).await {
            Ok(Some(user_id)) => UserId::Id(user_id),
            Ok(None) => return require_link(&orig).await,
            Err(err) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let user_args = UserArgs::rosu_id(&user_id, GameMode::Osu).await;

    let user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let daily = &user.daily_challenge;

    let len_daily_current = daily.daily_streak_current.to_string().len();
    let len_daily_best = daily.daily_streak_best.to_string().len();
    let daily_len = "Daily".len().max(len_daily_current).max(len_daily_best);

    let streaks = format!(
        "```
Streaks | {:^daily_len$} | Weekly
--------+-------+-------
Current | {:^daily_len$} | {:^6}
Best    | {:^daily_len$} | {:^6}
```",
        "Daily",
        daily.daily_streak_current.to_native(),
        daily.weekly_streak_current.to_native(),
        daily.daily_streak_best.to_native(),
        daily.weekly_streak_best.to_native(),
    );

    let fields = fields![
        "Daily challenge statistics", streaks, false;
        "Top 10%", daily.top_10p_placements.to_string(), true;
        "Top 50%", daily.top_50p_placements.to_string(), true;
        "Total", daily.playcount.to_string(), true;
    ];

    let played_today = daily.last_update.as_ref().is_some_and(|datetime| {
        let datetime = datetime.try_deserialize::<Panic>().always_ok();

        datetime.day() == OffsetDateTime::now_utc().day()
    });

    let embed = EmbedBuilder::new()
        .author(user.author_builder())
        .fields(fields)
        .footer(FooterBuilder::new(format!("Played today: {played_today}")))
        .thumbnail(user.avatar_url.as_ref());

    let builder = MessageBuilder::new().embed(embed);
    command.update(builder).await?;

    Ok(())
}
