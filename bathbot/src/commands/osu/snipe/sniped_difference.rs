use std::{cmp::Reverse, sync::Arc};

use bathbot_macros::command;
use bathbot_util::{
    constants::{GENERAL_ISSUE, HUISMETBENEN_ISSUE, OSU_API_ISSUE},
    matcher, IntHasher, MessageBuilder,
};
use eyre::{Report, Result};
use hashbrown::HashMap;
use rosu_v2::{prelude::OsuError, request::UserId};
use time::{Duration, OffsetDateTime};

use crate::{
    commands::osu::require_link,
    core::commands::CommandOrigin,
    manager::redis::{osu::UserArgs, RedisData},
    pagination::SnipedDiffPagination,
    Context,
};

use super::{SnipePlayerGain, SnipePlayerLoss};

#[command]
#[desc("Display a user's recently acquired national #1 scores")]
#[help(
    "Display a user's national #1 scores that they acquired within the last week.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("sg", "snipegain", "snipesgain")]
#[group(Osu)]
async fn prefix_snipedgain(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> Result<()> {
    let args = match args.next() {
        Some(arg) => match matcher::get_mention_user(arg) {
            Some(id) => SnipePlayerGain {
                name: None,
                discord: Some(id),
            },
            None => SnipePlayerGain {
                name: Some(arg.into()),
                discord: None,
            },
        },
        None => SnipePlayerGain::default(),
    };

    player_gain(ctx, msg.into(), args).await
}

#[command]
#[desc("Display a user's recently lost national #1 scores")]
#[help(
    "Display a user's national #1 scores that they lost within the last week.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases(
    "sl",
    "snipeloss",
    "snipesloss",
    "snipedlost",
    "snipelost",
    "snipeslost"
)]
#[group(Osu)]
async fn prefix_snipedloss(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> Result<()> {
    let args = match args.next() {
        Some(arg) => match matcher::get_mention_user(arg) {
            Some(id) => SnipePlayerLoss {
                name: None,
                discord: Some(id),
            },
            None => SnipePlayerLoss {
                name: Some(arg.into()),
                discord: None,
            },
        },
        None => SnipePlayerLoss::default(),
    };

    player_loss(ctx, msg.into(), args).await
}

pub(super) async fn player_gain(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: SnipePlayerGain<'_>,
) -> Result<()> {
    let user_id = user_id!(ctx, orig, args);

    sniped_diff(ctx, orig, Difference::Gain, user_id).await
}

pub(super) async fn player_loss(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: SnipePlayerLoss<'_>,
) -> Result<()> {
    let user_id = user_id!(ctx, orig, args);

    sniped_diff(ctx, orig, Difference::Loss, user_id).await
}

async fn sniped_diff(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    diff: Difference,
    user_id: Option<UserId>,
) -> Result<()> {
    let user_id = match user_id {
        Some(user_id) => user_id,
        None => match ctx.user_config().osu_id(orig.user_id()?).await {
            Ok(Some(user_id)) => UserId::Id(user_id),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    // Request the user
    let user_args = UserArgs::rosu_id(&ctx, &user_id).await;

    let user = match ctx.redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = match user_id {
                UserId::Id(user_id) => format!("User with id {user_id} was not found"),
                UserId::Name(name) => format!("User `{name}` was not found"),
            };

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
    };

    let (country_code, username, user_id) = match &user {
        RedisData::Original(user) => {
            let country_code = user.country_code.as_str();
            let username = user.username.as_str();
            let user_id = user.user_id;

            (country_code, username, user_id)
        }
        RedisData::Archive(user) => {
            let country_code = user.country_code.as_str();
            let username = user.username.as_str();
            let user_id = user.user_id;

            (country_code, username, user_id)
        }
    };

    if !ctx.huismetbenen().is_supported(country_code).await {
        let content = format!("`{username}`'s country {country_code} is not supported :(");

        return orig.error(&ctx, content).await;
    }

    let client = &ctx.client();
    let now = OffsetDateTime::now_utc();
    let week_ago = now - Duration::weeks(1);

    // Request the scores
    let scores_fut = match diff {
        Difference::Gain => client.get_national_snipes(user_id, true, week_ago, now),
        Difference::Loss => client.get_national_snipes(user_id, false, week_ago, now),
    };

    let mut scores = match scores_fut.await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = orig.error(&ctx, HUISMETBENEN_ISSUE).await;

            return Err(err.wrap_err("failed to get snipes"));
        }
    };

    if scores.is_empty() {
        let content = format!(
            "`{username}` didn't {diff} national #1s in the last week.",
            diff = match diff {
                Difference::Gain => "gain any new",
                Difference::Loss => "lose any",
            }
        );

        let builder = MessageBuilder::new().embed(content);
        orig.create_message(&ctx, &builder).await?;

        return Ok(());
    }

    scores.sort_unstable_by_key(|s| Reverse(s.date));
    let maps = HashMap::with_hasher(IntHasher);

    SnipedDiffPagination::builder(user, diff, scores, maps)
        .start_by_update()
        .defer_components()
        .start(ctx, orig)
        .await
}

#[derive(Copy, Clone)]
pub enum Difference {
    Gain,
    Loss,
}
