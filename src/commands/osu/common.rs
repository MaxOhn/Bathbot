use crate::{
    arguments::{Args, MultNameArgs},
    embeds::{CommonEmbed, EmbedData},
    pagination::{CommonPagination, Pagination},
    tracking::process_tracking,
    util::{constants::OSU_API_ISSUE, get_combined_thumbnail, MessageExt},
    BotResult, Context, Name,
};

use futures::stream::{FuturesOrdered, StreamExt};
use hashbrown::HashSet;
use itertools::Itertools;
use rosu_v2::prelude::{GameMode, OsuError, Score};
use smallvec::SmallVec;
use std::{cmp::Ordering, fmt::Write, sync::Arc};
use twilight_model::channel::Message;

macro_rules! user_id {
    ($scores:ident[$idx:literal]) => {
        $scores[$idx].user.as_ref().unwrap().user_id
    };
}

type CommonUsers = SmallVec<[CommonUser; 3]>;

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

    let count = names.len();

    // Retrieve each user's top scores
    let mut scores_futs = names
        .into_iter()
        .map(|name| async {
            let scores_fut = ctx
                .osu()
                .user_scores(name.as_str())
                .limit(100)
                .mode(mode)
                .best()
                .await;

            (name, scores_fut)
        })
        .collect::<FuturesOrdered<_>>();

    let mut all_scores = Vec::<Vec<_>>::with_capacity(count);
    let mut users = CommonUsers::with_capacity(count);

    while let Some((mut name, result)) = scores_futs.next().await {
        match result {
            Ok(scores) => {
                if let Some(user_id) = scores.first().map(|s| s.user_id) {
                    name.make_ascii_lowercase();

                    users.push(CommonUser::new(name, user_id));
                } else {
                    let content = format!("User `{}` has no {} top scores", name, mode);

                    return msg.error(&ctx, content).await;
                }

                all_scores.push(scores);
            }
            Err(OsuError::NotFound) => {
                let content = format!("User `{}` was not found", name);

                return msg.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        }
    }

    drop(scores_futs);

    // Check if different names that both belong to the same user were given
    if users.iter().unique_by(|user| user.id()).count() == 1 {
        let content = "Give at least two different users";

        return msg.error(&ctx, content).await;
    }

    // Process users and their top scores for tracking
    for scores in all_scores.iter_mut() {
        process_tracking(&ctx, mode, scores, None).await;
    }

    // Consider only scores on common maps
    let mut map_ids: HashSet<u32> = all_scores
        .iter()
        .map(|scores| scores.iter().flat_map(|s| map_id!(s)))
        .flatten()
        .collect();

    map_ids.retain(|&id| {
        all_scores.iter().all(|scores| {
            scores
                .iter()
                .filter_map(|s| map_id!(s))
                .any(|map_id| map_id == id)
        })
    });

    all_scores
        .iter_mut()
        .for_each(|scores| scores.retain(|s| map_ids.contains(&map_id!(s).unwrap())));

    // Flatten scores, sort by beatmap id, then group by beatmap id
    let mut all_scores: Vec<Score> = all_scores.into_iter().flatten().collect();
    all_scores.sort_unstable_by_key(|score| map_id!(score));

    let mut scores_per_map: Vec<SmallVec<[(usize, f32, Score); 3]>> = all_scores
        .into_iter()
        .group_by(|score| map_id!(score))
        .into_iter()
        .map(|(_, scores)| {
            // Sort with respect to order of names
            let mut scores: Vec<Score> = scores.collect();

            if user_id!(scores[0]) != users[0].id() {
                let target = (user_id!(scores[1]) != users[0].id()) as usize + 1;
                scores.swap(0, target);
            }

            if user_id!(scores[1]) != users[1].id() {
                scores.swap(1, 2);
            }

            let mut scores: SmallVec<[(usize, f32, Score); 3]> = scores
                .into_iter()
                .map(|score| (0, score.pp.unwrap(), score))
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

            scores
        })
        .collect();

    // Sort the maps by their score's avg pp values
    scores_per_map.sort_unstable_by(|s1, s2| {
        let s1 = s1.iter().map(|(_, pp, _)| *pp).sum::<f32>();
        let s2 = s2.iter().map(|(_, pp, _)| *pp).sum::<f32>();

        s2.partial_cmp(&s1).unwrap_or(Ordering::Equal)
    });

    // Accumulate all necessary data
    let mut content = String::with_capacity(16);
    let len = users.len();
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

    let amount_common = scores_per_map.len();

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
        let limit = scores_per_map.len().min(10);

        CommonEmbed::new(&users, &scores_per_map[..limit], 0)
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
    let embed = &[data.into_builder().build()];

    let m = ctx
        .http
        .create_message(msg.channel_id)
        .content(&content)?
        .embeds(embed)?;

    let response = match thumbnail {
        Some(bytes) => {
            m.files(&[("avatar_fuse.png", &bytes)])
                .exec()
                .await?
                .model()
                .await?
        }
        None => m.exec().await?.model().await?,
    };

    // Add maps of scores to DB
    let map_iter = scores_per_map
        .iter()
        .filter_map(|scores| scores.first())
        .map(|(_, _, score)| score);

    if let Err(why) = ctx.psql().store_scores_maps(map_iter).await {
        unwind_error!(warn, why, "Error while adding score maps to DB: {}")
    }

    // Skip pagination if too few entries
    if scores_per_map.len() <= 10 {
        return Ok(());
    }

    // Pagination
    let pagination = CommonPagination::new(response, users, scores_per_map);
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
    name: Name,
    user_id: u32,
    pub first_count: usize,
}

impl CommonUser {
    fn new(name: Name, user_id: u32) -> Self {
        Self {
            name,
            user_id,
            first_count: 0,
        }
    }
}

impl CommonUser {
    pub fn id(&self) -> u32 {
        self.user_id
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}
