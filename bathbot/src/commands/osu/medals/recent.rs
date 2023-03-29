use std::{cmp::Reverse, mem, sync::Arc};

use bathbot_macros::command;
use bathbot_model::rosu_v2::user::{MedalCompact as MedalCompactRkyv, User};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSEKAI_ISSUE, OSU_API_ISSUE},
    matcher, MessageBuilder,
};
use eyre::{Report, Result};
use rkyv::{
    with::{DeserializeWith, Map},
    Infallible,
};
use rosu_v2::{
    prelude::{MedalCompact, OsuError},
    request::UserId,
};
use time::OffsetDateTime;

use crate::{
    commands::osu::{require_link, user_not_found},
    core::commands::CommandOrigin,
    embeds::MedalEmbed,
    manager::redis::{osu::UserArgs, RedisData},
    pagination::MedalRecentPagination,
    Context,
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
async fn prefix_medalrecent(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> Result<()> {
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
) -> Result<()> {
    let owner = orig.user_id()?;

    let user_id = match user_id!(ctx, orig, args) {
        Some(user_id) => user_id,
        None => match ctx.user_config().osu_id(owner).await {
            Ok(Some(user_id)) => UserId::Id(user_id),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let user_args = UserArgs::rosu_id(&ctx, &user_id).await;
    let user_fut = ctx.redis().osu_user(user_args);
    let medals_fut = ctx.redis().medals();

    let (mut user, mut all_medals) = match tokio::join!(user_fut, medals_fut) {
        (Ok(user), Ok(medals)) => (user, medals.into_original()),
        (Err(OsuError::NotFound), _) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        (Err(err), _) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
        (_, Err(err)) => {
            let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached medals"));
        }
    };

    let mut user_medals = match user {
        RedisData::Original(ref mut user) => mem::take(&mut user.medals),
        RedisData::Archive(ref user) => {
            Map::<MedalCompactRkyv>::deserialize_with(&user.medals, &mut Infallible).unwrap()
        }
    };

    if user_medals.is_empty() {
        let content = format!("`{}` has not achieved any medals yet :(", user.username());
        let builder = MessageBuilder::new().embed(content);
        orig.create_message(&ctx, &builder).await?;

        return Ok(());
    }

    user_medals.sort_unstable_by_key(|medal| Reverse(medal.achieved_at));
    let index = args.index.unwrap_or(1);

    let (medal_id, achieved_at) = match user_medals.get(index - 1) {
        Some(MedalCompact {
            medal_id,
            achieved_at,
        }) => (*medal_id, *achieved_at),
        None => {
            let content = format!(
                "`{}` only has {} medals, cannot show medal #{index}",
                user.username(),
                user_medals.len(),
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
        medal_count: user_medals.len(),
    };

    let embed_data = MedalEmbed::new(medal.clone(), Some(achieved), Vec::new(), None);

    let builder =
        MedalRecentPagination::builder(user, medal, user_medals, index, embed_data, all_medals);

    builder
        .start_by_update()
        .content(content)
        .start(ctx, orig)
        .await
}

pub struct MedalAchieved<'u> {
    pub user: &'u RedisData<User>,
    pub achieved_at: OffsetDateTime,
    pub index: usize,
    pub medal_count: usize,
}
