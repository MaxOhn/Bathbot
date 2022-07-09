use std::{cmp::Reverse, sync::Arc};

use command_macros::command;
use rosu_v2::prelude::{GameMode, MedalCompact, OsuError, User};
use time::OffsetDateTime;

use crate::{
    commands::osu::{get_user, require_link, UserArgs},
    core::commands::CommandOrigin,
    embeds::MedalEmbed,
    pagination::MedalRecentPagination,
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSEKAI_ISSUE, OSU_API_ISSUE},
        matcher,
    },
    BotResult, Context,
};

use super::MedalRecent;

#[command]
#[desc("Display a recently acquired medal of a user")]
#[help(
    "Display a recently acquired medal of a user.\n\
    To start from a certain index, specify a number right after the command, e.g. `mr3`."
)]
#[usage("[username]")]
#[examples("badewanne3", r#""im a fancy lad""#)]
#[aliases("mr", "recentmedal")]
#[group(AllModes)]
async fn prefix_medalrecent(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> BotResult<()> {
    let mut args_ = MedalRecent {
        index: args.num.map(|n| n as usize),
        ..Default::default()
    };

    if let Some(arg) = args.next() {
        if let Some(id) = matcher::get_mention_user(arg) {
            args_.discord = Some(id);
        } else {
            args_.name = Some(arg.into());
        }
    }

    recent(ctx, msg.into(), args_).await
}

pub(super) async fn recent(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: MedalRecent<'_>,
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

    let user_args = UserArgs::new(name.as_str(), GameMode::Osu);
    let user_fut = get_user(&ctx, &user_args);
    let redis = ctx.redis();

    let (mut user, mut all_medals) = match tokio::join!(user_fut, redis.medals()) {
        (Ok(user), Ok(medals)) => (user, medals.to_inner()),
        (Err(OsuError::NotFound), _) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        (Err(err), _) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
        (_, Err(err)) => {
            let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
        }
    };

    let mut achieved_medals = user.medals.take().unwrap_or_default();

    if achieved_medals.is_empty() {
        let content = format!("`{}` has not achieved any medals yet :(", user.username);
        let builder = MessageBuilder::new().embed(content);
        orig.create_message(&ctx, &builder).await?;

        return Ok(());
    }

    achieved_medals.sort_unstable_by_key(|medal| Reverse(medal.achieved_at));
    let index = args.index.unwrap_or(1);

    let (medal_id, achieved_at) = match achieved_medals.get(index - 1) {
        Some(MedalCompact {
            medal_id,
            achieved_at,
        }) => (*medal_id, *achieved_at),
        None => {
            let content = format!(
                "`{}` only has {} medals, cannot show medal #{index}",
                user.username,
                achieved_medals.len(),
            );

            return orig.error(&ctx, content).await;
        }
    };

    let medal = match all_medals.iter().position(|m| m.medal_id == medal_id) {
        Some(idx) => all_medals.swap_remove(idx),
        None => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            bail!("No medal with id `{medal_id}`");
        }
    };

    let content = "Most recent medals:";

    let achieved = MedalAchieved {
        user: &user,
        achieved_at,
        index,
        medal_count: achieved_medals.len(),
    };

    let embed_data = MedalEmbed::new(medal.clone(), Some(achieved), Vec::new(), None);

    let builder =
        MedalRecentPagination::builder(user, medal, achieved_medals, index, embed_data, all_medals);

    builder
        .start_by_update()
        .content(content)
        .start(ctx, orig)
        .await
}

pub struct MedalAchieved<'u> {
    pub user: &'u User,
    pub achieved_at: OffsetDateTime,
    pub index: usize,
    pub medal_count: usize,
}
