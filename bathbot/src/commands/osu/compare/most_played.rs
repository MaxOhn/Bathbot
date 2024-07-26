use std::{cmp::Reverse, collections::HashMap, fmt::Write};

use bathbot_macros::command;
use bathbot_model::rosu_v2::user::User;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher, IntHasher, MessageBuilder,
};
use eyre::{Report, Result};
use rosu_v2::{
    model::GameMode,
    prelude::{MostPlayedMap, OsuError},
    request::UserId,
    OsuResult,
};

use super::{CompareMostPlayed, AT_LEAST_ONE};
use crate::{
    active::{impls::CompareMostPlayedPagination, ActiveMessages},
    commands::osu::{user_not_found, UserExtraction},
    core::commands::CommandOrigin,
    manager::redis::{osu::UserArgs, RedisData},
    Context,
};

#[command]
#[desc("Compare the 100 most played maps of two users")]
#[help(
    "Compare the users' 100 most played maps and check which \
     ones appear for each user"
)]
#[usage("[name1] [name2]")]
#[example("badewanne3 \"nathan on osu\"")]
#[aliases("commonmostplayed", "mpc")]
#[group(AllModes)]
async fn prefix_mostplayedcommon(msg: &Message, args: Args<'_>) -> Result<()> {
    let mut args_ = CompareMostPlayed::default();

    for arg in args.take(2) {
        if let Some(id) = matcher::get_mention_user(arg) {
            if args_.discord1.is_none() {
                args_.discord1 = Some(id);
            } else {
                args_.discord2 = Some(id);
            }
        } else if args_.name1.is_none() {
            args_.name1 = Some(arg.into());
        } else {
            args_.name2 = Some(arg.into());
        }
    }

    mostplayed(msg.into(), args_).await
}

async fn extract_user_id(args: &mut CompareMostPlayed<'_>) -> UserExtraction {
    if let Some(name) = args.name1.take().or_else(|| args.name2.take()) {
        UserExtraction::Id(UserId::Name(name.as_ref().into()))
    } else if let Some(discord) = args.discord1.take().or_else(|| args.discord2.take()) {
        match Context::user_config().osu_id(discord).await {
            Ok(Some(user_id)) => UserExtraction::Id(UserId::Id(user_id)),
            Ok(None) => {
                UserExtraction::Content(format!("<@{discord}> is not linked to an osu!profile"))
            }
            Err(err) => UserExtraction::Err(err),
        }
    } else {
        UserExtraction::None
    }
}

pub(super) async fn mostplayed(
    orig: CommandOrigin<'_>,
    mut args: CompareMostPlayed<'_>,
) -> Result<()> {
    let owner = orig.user_id()?;

    let user_id1 = match extract_user_id(&mut args).await {
        UserExtraction::Id(user_id) => user_id,
        UserExtraction::Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err);
        }
        UserExtraction::Content(content) => return orig.error(content).await,
        UserExtraction::None => return orig.error(AT_LEAST_ONE).await,
    };

    let user_id2 = match extract_user_id(&mut args).await {
        UserExtraction::Id(user_id) => user_id,
        UserExtraction::Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err);
        }
        UserExtraction::Content(content) => return orig.error(content).await,
        UserExtraction::None => match Context::user_config().osu_id(owner).await {
            Ok(Some(user_id)) => UserId::Id(user_id),
            Ok(None) => {
                let content =
                    "Since you're not linked with the `/link` command, you must specify two names.";

                return orig.error(content).await;
            }
            Err(err) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let fut1 = get_user_and_scores(&user_id1);
    let fut2 = get_user_and_scores(&user_id2);

    let (user1, maps1, user2, maps2) = match tokio::join!(fut1, fut2) {
        (Ok((user1, maps1)), Ok((user2, maps2))) => (user1, maps1, user2, maps2),
        (Err(OsuError::NotFound), _) => {
            let content = user_not_found(user_id1).await;

            return orig.error(content).await;
        }
        (_, Err(OsuError::NotFound)) => {
            let content = user_not_found(user_id2).await;

            return orig.error(content).await;
        }
        (Err(err), _) | (_, Err(err)) => {
            let _ = orig.error(OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get scores");

            return Err(err);
        }
    };

    // Consider only maps that appear in each users map list
    let mut maps: HashMap<_, _, IntHasher> = maps1
        .into_iter()
        .map(|map| (map.map.map_id, ([map.count, 0], map)))
        .collect();

    for map in maps2 {
        if let Some(([_, count], _)) = maps.get_mut(&map.map.map_id) {
            *count += map.count;
        }
    }

    maps.retain(|_, ([_, b], _)| *b > 0);

    // Sort maps by sum of counts
    let mut map_counts: Vec<_> = maps
        .iter()
        .map(|(map_id, ([a, b], _))| (*map_id, a + b))
        .collect();

    map_counts.sort_unstable_by_key(|(_, count)| Reverse(*count));

    let amount_common = maps.len();

    // Accumulate all necessary data
    let mut content = format!("`{}` and `{}`", user1.username(), user2.username());

    if amount_common == 0 {
        content.push_str(" don't share any maps in their 100 most played maps");
        let builder = MessageBuilder::new().embed(content);
        orig.create_message(builder).await?;

        return Ok(());
    }

    let _ = write!(
        content,
        " have {amount_common}/100 common most played map{}",
        if amount_common > 1 { "s" } else { "" }
    );

    let pagination = CompareMostPlayedPagination::builder()
        .username1(user1.username().into())
        .username2(user2.username().into())
        .maps(maps)
        .map_counts(map_counts.into_boxed_slice())
        .content(content.into_boxed_str())
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

async fn get_user_and_scores(user_id: &UserId) -> OsuResult<(RedisData<User>, Vec<MostPlayedMap>)> {
    match UserArgs::rosu_id(user_id, GameMode::Osu).await {
        UserArgs::Args(args) => {
            let score_fut = Context::osu().user_most_played(args.user_id).limit(100);
            let user_fut = Context::redis().osu_user_from_args(args);

            tokio::try_join!(user_fut, score_fut)
        }
        UserArgs::User { user, .. } => Context::osu()
            .user_most_played(user.user_id)
            .limit(100)
            .await
            .map(|scores| (RedisData::Original(*user), scores)),
        UserArgs::Err(err) => Err(err),
    }
}
