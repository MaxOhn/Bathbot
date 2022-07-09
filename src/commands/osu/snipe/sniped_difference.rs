use std::{cmp::Reverse, sync::Arc};

use command_macros::command;
use hashbrown::HashMap;
use rosu_v2::prelude::{GameMode, OsuError, Username};
use time::{Duration, OffsetDateTime};

use crate::{
    commands::osu::{get_user, require_link, UserArgs},
    core::commands::CommandOrigin,
    pagination::SnipedDiffPagination,
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, HUISMETBENEN_ISSUE, OSU_API_ISSUE},
        matcher,
    },
    BotResult, Context,
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
async fn prefix_snipedgain(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> BotResult<()> {
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
async fn prefix_snipedloss(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> BotResult<()> {
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
) -> BotResult<()> {
    let name = username!(ctx, orig, args);

    sniped_diff(ctx, orig, Difference::Gain, name).await
}

pub(super) async fn player_loss(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: SnipePlayerLoss<'_>,
) -> BotResult<()> {
    let name = username!(ctx, orig, args);

    sniped_diff(ctx, orig, Difference::Loss, name).await
}

async fn sniped_diff(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    diff: Difference,
    name: Option<Username>,
) -> BotResult<()> {
    let name = match name {
        Some(name) => name,
        None => match ctx.psql().get_user_osu(orig.user_id()?).await {
            Ok(Some(osu)) => osu.into_username(),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    // Request the user
    let user_args = UserArgs::new(name.as_str(), GameMode::Osu);

    let mut user = match get_user(&ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    // Overwrite default mode
    user.mode = GameMode::Osu;

    if !ctx.contains_country(user.country_code.as_str()) {
        let content = format!(
            "`{}`'s country {} is not supported :(",
            user.username, user.country_code
        );

        return orig.error(&ctx, content).await;
    }

    let client = &ctx.client();
    let now = OffsetDateTime::now_utc();
    let week_ago = now - Duration::weeks(1);

    // Request the scores
    let scores_fut = match diff {
        Difference::Gain => client.get_national_snipes(&user, true, week_ago, now),
        Difference::Loss => client.get_national_snipes(&user, false, week_ago, now),
    };

    let mut scores = match scores_fut.await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = orig.error(&ctx, HUISMETBENEN_ISSUE).await;

            return Err(err.into());
        }
    };

    if scores.is_empty() {
        let content = format!(
            "`{name}` didn't {diff} national #1s in the last week.",
            name = user.username,
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
    let maps = HashMap::new();

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
