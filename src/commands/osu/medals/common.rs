use std::{
    cmp::{Ordering, Reverse},
    sync::Arc,
};

use command_macros::command;
use eyre::{Report, Result};
use hashbrown::HashMap;
use rosu_v2::prelude::{GameMode, User};
use time::OffsetDateTime;

use crate::{
    commands::osu::{get_user, NameExtraction, UserArgs},
    core::commands::CommandOrigin,
    custom_client::{MedalGroup, OsekaiMedal, Rarity},
    embeds::MedalsCommonUser,
    pagination::MedalsCommonPagination,
    util::{
        constants::{GENERAL_ISSUE, OSEKAI_ISSUE, OSU_API_ISSUE},
        get_combined_thumbnail,
        hasher::IntHasher,
        matcher,
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

async fn extract_name<'a>(ctx: &Context, args: &mut MedalCommon<'a>) -> NameExtraction {
    if let Some(name) = args.name1.take().or_else(|| args.name2.take()) {
        NameExtraction::Name(name.as_ref().into())
    } else if let Some(discord) = args.discord1.take().or_else(|| args.discord2.take()) {
        match ctx.psql().get_user_osu(discord).await {
            Ok(Some(osu)) => NameExtraction::Name(osu.into_username()),
            Ok(None) => {
                NameExtraction::Content(format!("<@{discord}> is not linked to an osu!profile"))
            }
            Err(err) => NameExtraction::Err(err.wrap_err("failed to get username")),
        }
    } else {
        NameExtraction::None
    }
}

pub(super) async fn common(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    mut args: MedalCommon<'_>,
) -> Result<()> {
    let name1 = match extract_name(&ctx, &mut args).await {
        NameExtraction::Name(name) => name,
        NameExtraction::Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
        NameExtraction::Content(content) => return orig.error(&ctx, content).await,
        NameExtraction::None => {
            let content = "You need to specify at least one osu username. \
            If you're not linked, you must specify two names.";

            return orig.error(&ctx, content).await;
        }
    };

    let name2 = match extract_name(&ctx, &mut args).await {
        NameExtraction::Name(name) => name,
        NameExtraction::Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
        NameExtraction::Content(content) => return orig.error(&ctx, content).await,
        NameExtraction::None => match ctx.psql().get_user_osu(orig.user_id()?).await {
            Ok(Some(osu)) => osu.into_username(),
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

    if name1 == name2 {
        return orig.error(&ctx, "Give two different names").await;
    }

    let MedalCommon { sort, filter, .. } = args;

    // Retrieve all users and their scores
    let user_args1 = UserArgs::new(name1.as_ref(), GameMode::Osu);
    let user_fut1 = get_user(&ctx, &user_args1);

    let user_args2 = UserArgs::new(name2.as_ref(), GameMode::Osu);
    let user_fut2 = get_user(&ctx, &user_args2);
    let redis = ctx.redis();

    let (user1, user2, mut all_medals) = match tokio::join!(user_fut1, user_fut2, redis.medals()) {
        (Ok(user1), Ok(user2), Ok(medals)) => (user1, user2, medals.to_inner()),
        (Err(err), ..) | (_, Err(err), _) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
        (.., Err(err)) => {
            let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached medals"));
        }
    };

    if user1.user_id == user2.user_id {
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
                        let rarities: HashMap<_, _> = rarities
                            .get()
                            .iter()
                            .map(|entry| (entry.medal_id, entry.possession_percent))
                            .collect();

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

    // Create the thumbnail
    let urls = [user1.avatar_url.as_str(), user2.avatar_url.as_str()];

    let thumbnail = match get_combined_thumbnail(&ctx, urls, 2, None).await {
        Ok(thumbnail) => Some(thumbnail),
        Err(err) => {
            warn!("{:?}", err.wrap_err("Failed to combine avatars"));

            None
        }
    };

    let user1 = MedalsCommonUser::new(user1.username, winner1);
    let user2 = MedalsCommonUser::new(user2.username, winner2);

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

fn extract_medals(user: &User) -> HashMap<u32, OffsetDateTime, IntHasher> {
    match user.medals.as_ref() {
        Some(medals) => medals
            .iter()
            .map(|medal| (medal.medal_id, medal.achieved_at))
            .collect(),
        None => HashMap::default(),
    }
}
