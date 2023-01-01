use std::{
    borrow::Cow,
    cmp::{Ordering, Reverse},
    sync::Arc,
};

use bathbot_macros::command;
use eyre::{Report, Result};
use hashbrown::HashMap;
use rkyv::{with::DeserializeWith, Infallible};
use rosu_v2::{prelude::OsuError, request::UserId};
use time::OffsetDateTime;

use crate::{
    commands::osu::UserExtraction,
    core::commands::CommandOrigin,
    custom_client::{MedalGroup, OsekaiMedal, Rarity},
    embeds::MedalsCommonUser,
    manager::redis::{
        osu::{User, UserArgs},
        RedisData,
    },
    pagination::MedalsCommonPagination,
    util::{
        constants::{GENERAL_ISSUE, OSEKAI_ISSUE, OSU_API_ISSUE},
        get_combined_thumbnail,
        hasher::IntHasher,
        matcher,
        rkyv_impls::DateTimeWrapper,
    },
    Context,
};

use super::{MedalCommon, MedalCommonFilter, MedalCommonOrder};

#[command]
#[desc("Compare which of the given users achieved medals first")]
#[usage("[username1] [username2]")]
#[example("badewanne3 5joshi")]
#[alias("medalcommon")]
#[group(AllModes)]
pub async fn prefix_medalscommon(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let mut args_ = MedalCommon::default();

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

    common(ctx, msg.into(), args_).await
}

async fn extract_user_id<'a>(ctx: &Context, args: &mut MedalCommon<'a>) -> UserExtraction {
    if let Some(name) = args.name1.take().or_else(|| args.name2.take()) {
        let name = match name {
            Cow::Borrowed(name) => name.into(),
            Cow::Owned(name) => name.into(),
        };

        UserExtraction::Id(UserId::Name(name))
    } else if let Some(discord) = args.discord1.take().or_else(|| args.discord2.take()) {
        match ctx.user_config().osu_id(discord).await {
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

pub(super) async fn common(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    mut args: MedalCommon<'_>,
) -> Result<()> {
    let user_id1 = match extract_user_id(&ctx, &mut args).await {
        UserExtraction::Id(user_id) => user_id,
        UserExtraction::Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
        UserExtraction::Content(content) => return orig.error(&ctx, content).await,
        UserExtraction::None => {
            let content = "You need to specify at least one osu username. \
            If you're not linked, you must specify two names.";

            return orig.error(&ctx, content).await;
        }
    };

    let user_id2 = match extract_user_id(&ctx, &mut args).await {
        UserExtraction::Id(user_id) => user_id,
        UserExtraction::Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
        UserExtraction::Content(content) => return orig.error(&ctx, content).await,
        UserExtraction::None => match ctx.user_config().osu_id(orig.user_id()?).await {
            Ok(Some(user_id)) => UserId::Id(user_id),
            Ok(None) => {
                let content =
                    "Since you're not linked with the `/link` command, you must specify two names.";

                return orig.error(&ctx, content).await;
            }
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    if user_id1 == user_id2 {
        return orig.error(&ctx, "Give two different names").await;
    }

    let MedalCommon { sort, filter, .. } = args;

    // Retrieve all users and their scores
    let user_args = UserArgs::rosu_id(&ctx, &user_id1).await;
    let user_fut1 = ctx.redis().osu_user(user_args);

    let user_args = UserArgs::rosu_id(&ctx, &user_id2).await;
    let user_fut2 = ctx.redis().osu_user(user_args);

    let medals_fut = ctx.redis().medals();

    let (user_res1, user_res2, all_medals_res) = tokio::join!(user_fut1, user_fut2, medals_fut);

    let (user1, user2) = match (user_res1, user_res2) {
        (Ok(user1), Ok(user2)) => (user1, user2),
        (Err(OsuError::NotFound), _) | (_, Err(OsuError::NotFound)) => {
            let content = "At least one of the users was not found";

            return orig.error(&ctx, content).await;
        }
        (Err(err), _) | (_, Err(err)) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(Report::new(err).wrap_err("failed to get user"));
        }
    };

    let mut all_medals = match all_medals_res {
        Ok(medals) => medals.into_original(),
        Err(err) => {
            let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached medals"));
        }
    };

    if user1.user_id() == user2.user_id() {
        let content = "Give two different users";

        return orig.error(&ctx, content).await;
    }

    // Combining and sorting all medals
    let medals1 = extract_medals(&user1);
    let mut medals2 = extract_medals(&user2);

    let mut medals = Vec::with_capacity(all_medals.len());

    for (medal_id, achieved1) in medals1 {
        match all_medals.iter().position(|m| m.medal_id == medal_id) {
            Some(idx) => {
                let achieved2 = medals2.remove(&medal_id);

                let entry = MedalEntryCommon {
                    medal: all_medals.swap_remove(idx),
                    achieved1: Some(achieved1),
                    achieved2,
                };

                medals.push(entry);
            }
            None => warn!("Missing medal id {medal_id}"),
        }
    }

    for (medal_id, achieved2) in medals2 {
        match all_medals.iter().position(|m| m.medal_id == medal_id) {
            Some(idx) => {
                let entry = MedalEntryCommon {
                    medal: all_medals.swap_remove(idx),
                    achieved1: None,
                    achieved2: Some(achieved2),
                };

                medals.push(entry);
            }
            None => warn!("Missing medal id {medal_id}"),
        }
    }

    match filter {
        None | Some(MedalCommonFilter::None) => {}
        Some(MedalCommonFilter::Unique) => {
            medals.retain(|entry| entry.achieved1.is_none() || entry.achieved2.is_none())
        }
        Some(other) => {
            let group = match other {
                MedalCommonFilter::Skill => MedalGroup::Skill,
                MedalCommonFilter::Dedication => MedalGroup::Dedication,
                MedalCommonFilter::HushHush => MedalGroup::HushHush,
                MedalCommonFilter::BeatmapPacks => MedalGroup::BeatmapPacks,
                MedalCommonFilter::BeatmapChallengePacks => MedalGroup::BeatmapChallengePacks,
                MedalCommonFilter::SeasonalSpotlights => MedalGroup::SeasonalSpotlights,
                MedalCommonFilter::BeatmapSpotlights => MedalGroup::BeatmapSpotlights,
                MedalCommonFilter::ModIntroduction => MedalGroup::ModIntroduction,
                _ => unreachable!(),
            };

            medals.retain(|entry| entry.medal.grouping == group)
        }
    }

    match sort {
        Some(MedalCommonOrder::Alphabet) => {
            medals.sort_unstable_by(|a, b| a.medal.name.cmp(&b.medal.name))
        }
        Some(MedalCommonOrder::DateFirst) => {
            medals.sort_unstable_by_key(|entry| match (entry.achieved1, entry.achieved2) {
                (Some(a1), Some(a2)) => a1.min(a2),
                (Some(a1), None) => a1,
                (None, Some(a2)) => a2,
                (None, None) => unreachable!(),
            })
        }
        Some(MedalCommonOrder::DateLast) => {
            medals.sort_unstable_by_key(|entry| match (entry.achieved1, entry.achieved2) {
                (Some(a1), Some(a2)) => Reverse(a1.max(a2)),
                (Some(a1), None) => Reverse(a1),
                (None, Some(a2)) => Reverse(a2),
                (None, None) => unreachable!(),
            })
        }
        None => medals.sort_unstable_by(|a, b| a.medal.cmp(&b.medal)),
        Some(MedalCommonOrder::Rarity) => {
            if !medals.is_empty() {
                match ctx.redis().osekai_ranking::<Rarity>().await {
                    Ok(rarities) => {
                        let rarities: HashMap<_, _, IntHasher> = match rarities {
                            RedisData::Original(rarities) => rarities
                                .into_iter()
                                .map(|entry| (entry.medal_id, entry.possession_percent))
                                .collect(),
                            RedisData::Archived(rarities) => rarities
                                .iter()
                                .map(|entry| (entry.medal_id, entry.possession_percent))
                                .collect(),
                        };

                        medals.sort_unstable_by(|a, b| {
                            let rarity1 = rarities.get(&a.medal.medal_id).copied().unwrap_or(100.0);
                            let rarity2 = rarities.get(&b.medal.medal_id).copied().unwrap_or(100.0);

                            rarity1.partial_cmp(&rarity2).unwrap_or(Ordering::Equal)
                        });
                    }
                    Err(err) => {
                        let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

                        return Err(err.wrap_err("failed to get cached rarity ranking"));
                    }
                }
            }
        }
    }

    let mut winner1 = 0;
    let mut winner2 = 0;

    for entry in &medals {
        match (entry.achieved1, entry.achieved2) {
            (Some(a1), Some(a2)) => match a1 < a2 {
                true => winner1 += 1,
                false => winner2 += 1,
            },
            (Some(_), None) => winner1 += 1,
            (None, Some(_)) => winner2 += 1,
            (None, None) => unreachable!(),
        }
    }

    let urls = [user1.avatar_url(), user2.avatar_url()];

    let thumbnail = match get_combined_thumbnail(&ctx, urls, 2, None).await {
        Ok(thumbnail) => Some(thumbnail),
        Err(err) => {
            warn!("{:?}", err.wrap_err("Failed to combine avatars"));

            None
        }
    };

    let username1 = match user1 {
        RedisData::Original(user) => user.username,
        RedisData::Archived(user) => user.username.as_str().into(),
    };

    let username2 = match user2 {
        RedisData::Original(user) => user.username,
        RedisData::Archived(user) => user.username.as_str().into(),
    };

    let user1 = MedalsCommonUser::new(username1, winner1);
    let user2 = MedalsCommonUser::new(username2, winner2);

    let mut builder = MedalsCommonPagination::builder(user1, user2, medals);

    if let Some(bytes) = thumbnail {
        builder = builder.attachment("avatar_fuse.png", bytes);
    }

    builder.start_by_update().start(ctx, orig).await
}

pub struct MedalEntryCommon {
    pub medal: OsekaiMedal,
    pub achieved1: Option<OffsetDateTime>,
    pub achieved2: Option<OffsetDateTime>,
}

fn extract_medals(user: &RedisData<User>) -> HashMap<u32, OffsetDateTime, IntHasher> {
    match user {
        RedisData::Original(user) => user
            .medals
            .iter()
            .map(|medal| (medal.medal_id, medal.achieved_at))
            .collect(),
        RedisData::Archived(user) => user
            .medals
            .iter()
            .map(|medal| {
                let achieved_at =
                    DateTimeWrapper::deserialize_with(&medal.achieved_at, &mut Infallible).unwrap();

                (medal.medal_id, achieved_at)
            })
            .collect(),
    }
}
