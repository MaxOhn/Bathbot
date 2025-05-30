use bathbot_macros::command;
use bathbot_util::{
    EmbedBuilder, FooterBuilder, MessageBuilder, constants::GENERAL_ISSUE, fields, matcher,
};
use eyre::{Report, Result};
use rkyv::rancor::{Panic, ResultExt};
use rosu_v2::{error::OsuError, model::GameMode, request::UserId};
use time::OffsetDateTime;
use twilight_model::guild::Permissions;

use super::DailyChallengeUser;
use crate::{
    commands::osu::{daily_challenge::DC_USER_DESC, require_link, user_not_found},
    core::{
        Context,
        commands::{CommandOrigin, prefix::Args},
    },
    manager::redis::osu::{UserArgs, UserArgsError},
    util::CachedUserExt,
};

impl<'m> DailyChallengeUser<'m> {
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
#[desc(DC_USER_DESC)]
#[usage("[username]")]
#[examples("peppy")]
#[aliases("dcu", "dcuser", "dcp", "dcprofile", "dailychallengeprofile")]
#[group(AllModes)]
async fn prefix_dailychallengeuser(
    msg: &Message,
    args: Args<'_>,
    perms: Option<Permissions>,
) -> Result<()> {
    let orig = CommandOrigin::from_msg(msg, perms);
    let args = DailyChallengeUser::args(args);

    user(orig, args).await
}

pub(super) async fn user(orig: CommandOrigin<'_>, user: DailyChallengeUser<'_>) -> Result<()> {
    let owner = orig.user_id()?;

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

    let footer = format!("Played today: {}", if played_today { '✅' } else { '❌' });

    let embed = EmbedBuilder::new()
        .author(user.author_builder(false))
        .fields(fields)
        .footer(FooterBuilder::new(footer))
        .thumbnail(user.avatar_url.as_ref());

    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(builder).await?;

    Ok(())
}
