use std::{cmp::Reverse, collections::HashMap};

use bathbot_macros::command;
use bathbot_model::rosu_v2::user::MedalCompactRkyv;
use bathbot_psql::model::configs::HideSolutions;
use bathbot_util::{IntHasher, MessageBuilder, constants::GENERAL_ISSUE, matcher};
use eyre::{Report, Result};
use rand::{Rng, thread_rng};
use rkyv::{
    rancor::{Panic, ResultExt},
    with::{Map, With},
};
use rosu_v2::{
    model::GameMode,
    prelude::{MedalCompact, OsuError},
    request::UserId,
};
use time::OffsetDateTime;

use super::{MedalEmbed, MedalRecent};
use crate::{
    Context,
    active::{ActiveMessages, impls::MedalsRecentPagination},
    commands::osu::{require_link, user_not_found},
    core::commands::CommandOrigin,
    manager::redis::osu::{CachedUser, UserArgs, UserArgsError},
};

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
async fn prefix_medalrecent(msg: &Message, mut args: Args<'_>) -> Result<()> {
    let mut args_ = MedalRecent {
        index: args.num.to_string_opt().map(String::into),
        ..Default::default()
    };

    if let Some(arg) = args.next() {
        if let Some(id) = matcher::get_mention_user(arg) {
            args_.discord = Some(id);
        } else {
            args_.name = Some(arg.into());
        }
    }

    recent(msg.into(), args_).await
}

pub(super) async fn recent(orig: CommandOrigin<'_>, args: MedalRecent<'_>) -> Result<()> {
    let owner = orig.user_id()?;

    let user_id = match user_id!(orig, args) {
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
    let user_fut = Context::redis().osu_user(user_args);
    let medals_fut = Context::redis().medals();

    let (user, all_medals) = match tokio::join!(user_fut, medals_fut) {
        (Ok(user), Ok(medals)) => (user, medals),
        (Err(UserArgsError::Osu(OsuError::NotFound)), _) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        (Err(err), _) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let report = Report::new(err).wrap_err("Failed to get user");

            return Err(report);
        }
        (_, Err(err)) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get cached medals"));
        }
    };

    let mut user_medals = rkyv::api::deserialize_using::<_, _, Panic>(
        With::<_, Map<MedalCompactRkyv>>::cast(&user.medals),
        &mut (),
    )
    .always_ok();

    if user_medals.is_empty() {
        let content = format!(
            "`{}` has not achieved any medals yet :(",
            user.username.as_str()
        );
        let builder = MessageBuilder::new().embed(content);
        orig.create_message(builder).await?;

        return Ok(());
    }

    if let Some(group) = args.group {
        user_medals.retain(|medal| {
            all_medals
                .binary_search_by_key(&medal.medal_id, |medal| medal.medal_id.to_native())
                .is_ok_and(|idx| all_medals[idx].grouping == group)
        });
    }

    user_medals.sort_unstable_by_key(|medal| Reverse(medal.achieved_at));

    let index = match args.index.as_deref() {
        Some("random" | "?") => match user_medals.is_empty() {
            false => thread_rng().gen_range(0..user_medals.len()),
            true => 0,
        },
        Some(n) => match n.parse::<usize>() {
            Ok(n) => n.saturating_sub(1),
            Err(_) => {
                let content = format!(
                    "Failed to parse index. \
                    Must be an integer between 1 and {} or `random` / `?`.",
                    user_medals.len()
                );

                return orig.error(content).await;
            }
        },
        None => 0,
    };

    let (medal_id, achieved_at) = match user_medals.get(index) {
        Some(MedalCompact {
            medal_id,
            achieved_at,
        }) => (*medal_id, *achieved_at),
        None => {
            let content = format!(
                "`{}` only has {} medals{filtered}, cannot show medal #{index}",
                user.username.as_str(),
                user_medals.len(),
                filtered = if args.group.is_some() {
                    " of that group"
                } else {
                    ""
                },
                index = index + 1,
            );

            return orig.error(content).await;
        }
    };

    let medal = match all_medals.binary_search_by_key(&medal_id, |medal| medal.medal_id.to_native())
    {
        Ok(idx) => &all_medals[idx],
        Err(_) => {
            let _ = orig.error(GENERAL_ISSUE).await;

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

    let hide_solutions = match orig.guild_id() {
        Some(guild) => {
            Context::guild_config()
                .peek(guild, |config| {
                    config.hide_medal_solution.unwrap_or(HideSolutions::ShowAll)
                })
                .await
        }
        None => HideSolutions::ShowAll,
    };

    let embed_data = MedalEmbed::new(medal, Some(achieved), Vec::new(), None, hide_solutions);

    let mut embeds = HashMap::with_hasher(IntHasher);
    embeds.insert(index, embed_data);

    let mut pagination = MedalsRecentPagination::builder()
        .user(user)
        .achieved_medals(user_medals.into_boxed_slice())
        .embeds(embeds)
        .medals(all_medals)
        .hide_solutions(hide_solutions)
        .content(content)
        .msg_owner(owner)
        .build();

    pagination.set_index(index);

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

pub struct MedalAchieved<'u> {
    pub user: &'u CachedUser,
    pub achieved_at: OffsetDateTime,
    pub index: usize,
    pub medal_count: usize,
}
