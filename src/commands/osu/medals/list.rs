use std::{
    cmp::{Ordering, Reverse},
    mem,
    sync::Arc,
};

use chrono::{DateTime, Utc};
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{GameMode, OsuError};

use crate::{
    commands::osu::{get_user, require_link, UserArgs},
    core::commands::CommandOrigin,
    custom_client::{OsekaiMedal, Rarity},
    embeds::{EmbedData, MedalsListEmbed},
    pagination::{MedalsListPagination, Pagination},
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSEKAI_ISSUE, OSU_API_ISSUE},
        numbers,
    },
    BotResult, Context,
};

use super::{MedalList, MedalListOrder};

pub(super) async fn list(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: MedalList<'_>,
) -> BotResult<()> {
    let name = match username!(ctx, orig, args) {
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

    let MedalList {
        sort,
        group,
        reverse,
        ..
    } = args;

    let user_args = UserArgs::new(name.as_str(), GameMode::STD);
    let user_fut = get_user(&ctx, &user_args);
    let rarity_fut = ctx.client().get_osekai_ranking::<Rarity>();
    let redis = ctx.redis();

    let (mut user, mut osekai_medals, rarities) =
        match tokio::join!(user_fut, redis.medals(), rarity_fut) {
            (Ok(user), Ok(medals), Ok(rarities)) => (user, medals.to_inner(), rarities),
            (Err(OsuError::NotFound), ..) => {
                let content = format!("User `{name}` was not found");

                return orig.error(&ctx, content).await;
            }
            (Err(err), ..) => {
                let _ = orig.error(&ctx, OSU_API_ISSUE).await;

                return Err(err.into());
            }
            (_, Err(err), _) | (.., Err(err)) => {
                let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

                return Err(err.into());
            }
        };

    let rarities: HashMap<_, _> = rarities
        .into_iter()
        .map(|entry| (entry.medal_id, entry.possession_percent))
        .collect();

    let acquired = (
        user.medals.as_ref().map_or(0, Vec::len),
        osekai_medals.len(),
    );

    osekai_medals.sort_unstable_by_key(|medal| medal.medal_id);

    let mut medals = Vec::with_capacity(acquired.0);

    let medals_iter = user
        .medals
        .as_mut()
        .map_or_else(Vec::new, mem::take)
        .into_iter()
        .filter_map(|m| {
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

    medals.extend(medals_iter);

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

    let len = medals.len().min(10);
    let pages = numbers::div_euclid(10, medals.len());
    let embed_data = MedalsListEmbed::new(&user, &medals[..len], acquired, (1, pages));

    let content = match group {
        None => format!("All medals of `{name}` sorted by {reverse_str}{order_str}:"),
        Some(group) => {
            format!("All `{group}` medals of `{name}` sorted by {reverse_str}{order_str}:")
        }
    };

    let builder = MessageBuilder::new()
        .embed(embed_data.build())
        .content(content);

    let response_raw = orig.create_message(&ctx, &builder).await?;

    // Skip pagination if too few entries
    if medals.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = MedalsListPagination::new(response, user, medals, acquired);
    let owner = orig.user_id()?;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

pub struct MedalEntryList {
    pub medal: OsekaiMedal,
    pub achieved: DateTime<Utc>,
    pub rarity: f32,
}
