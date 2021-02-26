use super::ResultError;
use crate::{
    arguments::{Args, MultNameArgs},
    embeds::{CommonEmbed, EmbedData},
    pagination::{CommonPagination, Pagination},
    tracking::process_tracking,
    unwind_error,
    util::{constants::OSU_API_ISSUE, get_combined_thumbnail, MessageExt},
    BotResult, Context,
};

use futures::{
    future::{FutureExt, TryFutureExt},
    stream::{FuturesOrdered, FuturesUnordered, TryStreamExt},
};
use itertools::Itertools;
use rayon::prelude::*;
use rosu::model::{GameMode, Score, User};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fmt::Write,
    sync::Arc,
};
use twilight_model::channel::Message;

async fn common_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let mut args = MultNameArgs::new(&ctx, args, 3);

    let names = match args.names.len() {
        0 => {
            let content = "You need to specify at least one osu username. \
                If you're not linked, you must specify at least two names.";

            return msg.error(&ctx, content).await;
        }
        1 => match ctx.get_link(msg.author.id.0) {
            Some(name) => {
                args.names.insert(0, name);

                args.names
            }
            None => {
                let prefix = ctx.config_first_prefix(msg.guild_id);

                let content = format!(
                    "Since you're not linked via `{}link`, \
                    you must specify at least two names.",
                    prefix
                );

                return msg.error(&ctx, content).await;
            }
        },
        _ => args.names,
    };

    if names.iter().unique().count() == 1 {
        let content = "Give at least two different names";

        return msg.error(&ctx, content).await;
    }

    // Retrieve all users
    let user_futs = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            ctx.osu()
                .user(name.as_str())
                .mode(mode)
                .map(move |result| match result {
                    Ok(Some(user)) => Ok(user),
                    Ok(None) => Err(ResultError::None(i)),
                    Err(why) => Err(ResultError::Osu(why)),
                })
                .map_ok(CommonUser::from)
        })
        .collect::<FuturesOrdered<_>>()
        .try_collect();

    let mut users: Vec<CommonUser> = match user_futs.await {
        Ok(users) => users,
        Err(ResultError::None(idx)) => {
            let content = format!("User `{}` was not found", names[idx]);

            return msg.error(&ctx, content).await;
        }
        Err(ResultError::Osu(why)) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Check if different names were given
    // that both belong to the same user
    if users.iter().unique_by(|user| user.id()).count() == 1 {
        let content = "Give at least two different users";

        return msg.error(&ctx, content).await;
    }

    // Retrieve each user's top scores
    let score_futs = users
        .iter()
        .map(|u| {
            u.user
                .get_top_scores(ctx.osu())
                .limit(100)
                .mode(mode)
                .map_ok(move |scores| (u.user.user_id, scores))
        })
        .collect::<FuturesUnordered<_>>()
        .try_collect();

    let mut all_scores: Vec<(u32, Vec<Score>)> = match score_futs.await {
        Ok(all_scores) => all_scores,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Process users and their top scores for tracking
    {
        let mut maps = HashMap::new();

        for (u, (_, scores)) in users.iter().zip(all_scores.iter()) {
            process_tracking(&ctx, mode, scores, Some(&u.user), &mut maps).await;
        }
    }

    // Consider only scores on common maps
    let mut map_ids: HashSet<u32> = all_scores
        .iter()
        .map(|(_, scores)| scores.iter().flat_map(|s| s.beatmap_id))
        .flatten()
        .collect();

    map_ids.retain(|&id| {
        all_scores.iter().all(|(_, scores)| {
            scores
                .iter()
                .filter_map(|s| s.beatmap_id)
                .any(|map_id| map_id == id)
        })
    });

    all_scores
        .par_iter_mut()
        .for_each(|(_, scores)| scores.retain(|s| map_ids.contains(&s.beatmap_id.unwrap())));

    // Flatten scores, sort by beatmap id, then group by beatmap id
    let mut all_scores: Vec<(u32, Score)> = all_scores
        .into_iter()
        .map(|(user_id, scores)| scores.into_iter().map(move |score| (user_id, score)))
        .flatten()
        .collect();

    all_scores.sort_unstable_by_key(|(_, s)| s.beatmap_id);

    let all_scores: HashMap<u32, Vec<(usize, f32)>> = all_scores
        .into_iter()
        .group_by(|(_, score)| score.beatmap_id.unwrap())
        .into_iter()
        .map(|(map_id, scores)| {
            // Sort with respect to order of users
            let mut scores: Vec<(u32, Score)> = scores.collect();

            if scores[0].0 != users[0].id() {
                let target = (scores[1].0 != users[0].id()) as usize + 1;
                scores.swap(0, target);
            }

            if scores[1].0 != users[1].id() {
                scores.swap(1, 2);
            }

            let mut scores: Vec<_> = scores
                .into_iter()
                .flat_map(|(_, score)| score.pp)
                .map(|pp| (0, pp))
                .collect();

            // Calculate the index of the pp ordered by their values
            if scores[0].1 > scores[1].1 {
                scores[1].0 += 1;
            } else {
                scores[0].0 += 1;
            }

            if scores.len() == 3 {
                if scores[0].1 > scores[2].1 {
                    scores[2].0 += 1;
                } else {
                    scores[0].0 += 1;
                }

                if scores[1].1 > scores[2].1 {
                    scores[2].0 += 1;
                } else {
                    scores[1].0 += 1;
                }
            }

            if scores[0].0 == 0 {
                users[0].first_count += 1;
            } else if scores[1].0 == 0 {
                users[1].first_count += 1;
            } else {
                users[2].first_count += 1;
            }

            (map_id, scores)
        })
        .collect();

    // Sort the maps by their score's avg pp values
    let mut pp_avg: Vec<(u32, f32)> = all_scores
        .par_iter()
        .map(|(&map_id, scores)| {
            let sum = scores.iter().fold(0.0, |sum, (_, next)| sum + *next);

            (map_id, sum / scores.len() as f32)
        })
        .collect();

    pp_avg.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

    // Try retrieving all maps of common scores from the database
    let mut maps = {
        let map_id_vec = map_ids.iter().copied().collect_vec();

        match ctx.psql().get_beatmaps(&map_id_vec).await {
            Ok(maps) => maps,
            Err(why) => {
                unwind_error!(warn, why, "Error while getting maps from DB: {}");

                HashMap::default()
            }
        }
    };

    let amount_common = map_ids.len();
    map_ids.retain(|id| !maps.contains_key(id));

    // Retrieve all missing maps from the API
    let missing_maps = if map_ids.is_empty() {
        None
    } else {
        let map_futs = map_ids
            .into_iter()
            .map(|id| {
                ctx.osu()
                    .beatmap()
                    .map_id(id)
                    .map(move |result| match result {
                        Ok(Some(map)) => Ok(map),
                        Ok(None) => Err(ResultError::None(id)),
                        Err(why) => Err(ResultError::Osu(why)),
                    })
            })
            .collect::<FuturesUnordered<_>>()
            .try_collect::<Vec<_>>();

        match map_futs.await {
            Ok(missing_maps) => {
                for map in missing_maps.iter() {
                    maps.insert(map.beatmap_id, map.clone());
                }

                Some(missing_maps)
            }
            Err(ResultError::None(id)) => {
                let content = format!("API returned no result for map id {}", id);

                return msg.error(&ctx, content).await;
            }
            Err(ResultError::Osu(why)) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        }
    };

    // Combine maps and scores into one variable
    let map_scores = all_scores
        .into_iter()
        .filter_map(|(map_id, scores)| {
            let map = maps.remove(&map_id)?;

            Some((map_id, (map, scores)))
        })
        .collect();

    // Accumulate all necessary data
    let mut content = String::with_capacity(16);
    let len = names.len();
    let mut iter = users.iter().map(CommonUser::name);

    if let Some(first) = iter.next() {
        let last = iter.next_back();
        let _ = write!(content, "`{}`", first);

        for name in iter {
            let _ = write!(content, ", `{}`", name);
        }

        if let Some(name) = last {
            if len > 2 {
                content.push(',');
            }

            let _ = write!(content, " and `{}`", name);
        }
    }

    if amount_common == 0 {
        content.push_str(" have no common scores");
    } else {
        let _ = write!(
            content,
            " have {} common beatmap{} in their top 100",
            amount_common,
            if amount_common > 1 { "s" } else { "" }
        );
    }

    // Create the combined profile pictures
    let thumbnail_fut =
        async { get_combined_thumbnail(&ctx, users.iter().map(CommonUser::id)).await };

    let data_fut = async {
        let id_pps = &pp_avg[..10.min(pp_avg.len())];

        CommonEmbed::new(&users, &map_scores, id_pps, 0)
    };

    let (thumbnail_result, data) = tokio::join!(thumbnail_fut, data_fut);

    let thumbnail = match thumbnail_result {
        Ok(thumbnail) => Some(thumbnail),
        Err(why) => {
            unwind_error!(warn, why, "Error while combining avatars: {}");

            None
        }
    };

    // Creating the embed
    let embed = data.build().build()?;
    let mut m = ctx.http.create_message(msg.channel_id);

    m = match thumbnail {
        Some(bytes) => m.attachment("avatar_fuse.png", bytes),
        None => m,
    };

    let response = m.content(content)?.embed(embed)?.await?;

    // Add missing maps to database
    if let Some(maps) = missing_maps {
        match ctx.psql().insert_beatmaps(&maps).await {
            Ok(n) if n < 2 => {}
            Ok(n) => info!("Added {} maps to DB", n),
            Err(why) => unwind_error!(warn, why, "Error while adding maps to DB: {}"),
        }
    }

    // Skip pagination if too few entries
    if pp_avg.len() <= 10 {
        response.reaction_delete(&ctx, msg.author.id);

        return Ok(());
    }

    // Pagination
    let pagination = CommonPagination::new(response, users, map_scores, pp_avg);
    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (common): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Compare maps of players' top100s")]
#[long_desc(
    "Compare the users' top 100 and check which \
     maps appear in each top list (up to 3 users)"
)]
#[usage("[name1] [name2] [name3]")]
#[example("badewanne3 \"nathan on osu\" idke")]
pub async fn common(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    common_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Compare maps of players' top100s")]
#[long_desc(
    "Compare the mania users' top 100 and check which \
     maps appear in each top list (up to 3 users)"
)]
#[usage("[name1] [name2] [name3]")]
#[example("badewanne3 \"nathan on osu\" idke")]
#[aliases("commonm")]
pub async fn commonmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    common_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Compare maps of players' top100s")]
#[long_desc(
    "Compare the taiko users' top 100 and check which \
     maps appear in each top list (up to 3 users)"
)]
#[usage("[name1] [name2] [name3]")]
#[example("badewanne3 \"nathan on osu\" idke")]
#[aliases("commont")]
pub async fn commontaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    common_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Compare maps of players' top100s")]
#[long_desc(
    "Compare the ctb users' top 100 and check which \
     maps appear in each top list (up to 3 users)"
)]
#[usage("[name1] [name2] [name3]")]
#[example("badewanne3 \"nathan on osu\" idke")]
#[aliases("commonc")]
pub async fn commonctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    common_main(GameMode::CTB, ctx, msg, args).await
}

pub struct CommonUser {
    user: User,
    pub first_count: usize,
}

impl From<User> for CommonUser {
    #[inline]
    fn from(user: User) -> Self {
        Self {
            user,
            first_count: 0,
        }
    }
}

impl CommonUser {
    #[inline]
    fn id(&self) -> u32 {
        self.user.user_id
    }

    #[inline]
    pub fn name(&self) -> &str {
        self.user.username.as_str()
    }
}
