use std::{
    cmp::{Ordering, Reverse},
    sync::Arc,
};

use eyre::{Report, Result};
use hashbrown::HashMap;
use rkyv::{with::DeserializeWith, Infallible};
use rosu_v2::{prelude::OsuError, request::UserId};
use time::OffsetDateTime;

use crate::{
    commands::osu::{require_link, user_not_found},
    core::commands::CommandOrigin,
    custom_client::{OsekaiMedal, Rarity},
    manager::redis::{osu::UserArgs, RedisData},
    pagination::MedalsListPagination,
    util::{
        constants::{GENERAL_ISSUE, OSEKAI_ISSUE, OSU_API_ISSUE},
        hasher::IntHasher,
        rkyv_impls::DateTimeWrapper,
    },
    Context,
};

use super::{MedalList, MedalListOrder};

pub(super) async fn list(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: MedalList<'_>,
) -> Result<()> {
    let user_id = match user_id!(ctx, orig, args) {
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

    let MedalList {
        sort,
        group,
        reverse,
        ..
    } = args;

    let user_args = UserArgs::rosu_id(&ctx, &user_id).await;
    let user_fut = ctx.redis().osu_user(user_args);
    let medals_fut = ctx.redis().medals();
    let ranking_fut = ctx.redis().osekai_ranking::<Rarity>();

    let (mut user, mut osekai_medals, rarities) =
        match tokio::join!(user_fut, medals_fut, ranking_fut) {
            (Ok(user), Ok(medals), Ok(rarities)) => (user, medals.into_original(), rarities),
            (Err(OsuError::NotFound), ..) => {
                let content = user_not_found(&ctx, user_id).await;

                return orig.error(&ctx, content).await;
            }
            (Err(err), ..) => {
                let _ = orig.error(&ctx, OSU_API_ISSUE).await;
                let report = Report::new(err).wrap_err("failed to get user");

                return Err(report);
            }
            (_, Err(err), _) | (.., Err(err)) => {
                let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

                return Err(err.wrap_err("failed to get cached rarity ranking"));
            }
        };

    let rarities: HashMap<_, _, IntHasher> = match rarities {
        RedisData::Original(rarities) => rarities
            .iter()
            .map(|entry| (entry.medal_id, entry.possession_percent))
            .collect(),
        RedisData::Archived(rarities) => rarities
            .iter()
            .map(|entry| (entry.medal_id, entry.possession_percent))
            .collect(),
    };

    osekai_medals.sort_unstable_by_key(|medal| medal.medal_id);

    let (acquired, mut medals) = match user {
        RedisData::Original(ref mut user) => {
            let acquired = (user.medals.len(), osekai_medals.len());

            let medals_iter = user.medals.iter().filter_map(|m| {
                match osekai_medals
                    .iter()
                    .position(|m_| m_.medal_id == m.medal_id)
                {
                    Some(idx) => {
                        let entry = MedalEntryList {
                            medal: osekai_medals.swap_remove(idx),
                            achieved: m.achieved_at,
                            rarity: rarities.get(&m.medal_id).copied().unwrap_or(100.0),
                        };

                        Some(entry)
                    }
                    None => {
                        warn!("Missing medal id {}", m.medal_id);

                        None
                    }
                }
            });

            let mut medals = Vec::with_capacity(acquired.0);
            medals.extend(medals_iter);

            (acquired, medals)
        }
        RedisData::Archived(ref user) => {
            let acquired = (user.medals.len(), osekai_medals.len());

            let medals_iter = user.medals.iter().filter_map(|m| {
                match osekai_medals
                    .iter()
                    .position(|m_| m_.medal_id == m.medal_id)
                {
                    Some(idx) => {
                        let achieved_res =
                            DateTimeWrapper::deserialize_with(&m.achieved_at, &mut Infallible);

                        let entry = MedalEntryList {
                            medal: osekai_medals.swap_remove(idx),
                            achieved: achieved_res.unwrap(),
                            rarity: rarities.get(&m.medal_id).copied().unwrap_or(100.0),
                        };

                        Some(entry)
                    }
                    None => {
                        warn!("Missing medal id {}", m.medal_id);

                        None
                    }
                }
            });

            let mut medals = Vec::with_capacity(acquired.0);
            medals.extend(medals_iter);

            (acquired, medals)
        }
    };

    if let Some(group) = group {
        medals.retain(|entry| entry.medal.grouping == group);
    }

    let order_str = match sort.unwrap_or_default() {
        MedalListOrder::Alphabet => {
            medals.sort_unstable_by(|a, b| a.medal.name.cmp(&b.medal.name));

            "alphabet"
        }
        MedalListOrder::Date => {
            medals.sort_unstable_by_key(|entry| Reverse(entry.achieved));

            "date"
        }
        MedalListOrder::MedalId => {
            medals.sort_unstable_by_key(|entry| entry.medal.medal_id);

            "medal id"
        }
        MedalListOrder::Rarity => {
            medals.sort_unstable_by(|a, b| {
                a.rarity.partial_cmp(&b.rarity).unwrap_or(Ordering::Equal)
            });

            "rarity"
        }
    };

    let reverse_str = if reverse == Some(true) {
        medals.reverse();

        "reversed "
    } else {
        ""
    };

    let name = user.username();

    let content = match group {
        None => format!("All medals of `{name}` sorted by {reverse_str}{order_str}:",),
        Some(group) => {
            format!("All `{group}` medals of `{name}` sorted by {reverse_str}{order_str}:",)
        }
    };

    MedalsListPagination::builder(user, acquired, medals)
        .content(content)
        .start_by_update()
        .start(ctx, orig)
        .await
}

pub struct MedalEntryList {
    pub medal: OsekaiMedal,
    pub achieved: OffsetDateTime,
    pub rarity: f32,
}
