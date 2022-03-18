use std::{cmp::Ordering, fmt::Write, sync::Arc};

use eyre::Report;
use futures::stream::{FuturesOrdered, StreamExt};
use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use rosu_v2::prelude::{GameMode, OsuError, Score, Username};
use smallvec::SmallVec;

use crate::{
    commands::osu::{get_scores, ScoreArgs, UserArgs},
    embeds::{CommonEmbed, EmbedData},
    pagination::{CommonPagination, Pagination},
    tracking::process_osu_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        get_combined_thumbnail, MessageExt,
    },
    BotResult, CommandData, Context, MessageBuilder,
};

use super::TripleArgs;

macro_rules! user_id {
    ($scores:ident[$idx:literal]) => {
        $scores[$idx].user.as_ref().unwrap().user_id
    };
}

type CommonUsers = SmallVec<[CommonUser; 3]>;

pub(super) async fn _common(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: TripleArgs,
) -> BotResult<()> {
    let TripleArgs {
        name1,
        name2,
        name3,
        mode,
    } = args;

    let name1 = match name1 {
        Some(name) => name,
        None => {
            let content =
                "Since you're not linked with the `link` command, you must specify two names.";

            return data.error(&ctx, content).await;
        }
    };

    let mut names = Vec::with_capacity(3);
    names.push(name1);
    names.push(name2);

    if let Some(name) = name3 {
        names.push(name);
    }

    {
        let mut unique = HashMap::with_capacity(names.len());

        for name in names.iter() {
            *unique.entry(name.as_str()).or_insert(0) += 1;
        }

        if unique.len() == 1 {
            let content = "Give at least two different names";

            return data.error(&ctx, content).await;
        }
    }

    let count = names.len();

    // Retrieve each user's top scores
    let mut scores_futs: FuturesOrdered<_> = names
        .into_iter()
        .map(|name| async {
            let mut user_args = UserArgs::new(name.as_str(), mode);
            let score_args = ScoreArgs::top(100);
            let scores_fut = get_scores(&ctx, &user_args, &score_args);

            if let Some(alt_name) = user_args.whitespaced_name() {
                match scores_fut.await {
                    Ok(scores) => (name, Ok(scores)),
                    Err(OsuError::NotFound) => {
                        user_args.name = &alt_name;
                        let scores_result = get_scores(&ctx, &user_args, &score_args).await;

                        (alt_name.into(), scores_result)
                    }
                    Err(err) => (name, Err(err)),
                }
            } else {
                let scores_result = scores_fut.await;

                (name, scores_result)
            }
        })
        .collect();

    let mut all_scores = Vec::<Vec<_>>::with_capacity(count);
    let mut users = CommonUsers::with_capacity(count);

    while let Some((mut name, result)) = scores_futs.next().await {
        match result {
            Ok(scores) => {
                let opt = scores
                    .first()
                    .and_then(|s| Some((s.user_id, s.user.as_ref()?.avatar_url.clone())));

                if let Some((user_id, avatar_url)) = opt {
                    name.make_ascii_lowercase();

                    users.push(CommonUser::new(name, avatar_url, user_id));
                } else {
                    let content = format!("User `{name}` has no {mode} top scores");

                    return data.error(&ctx, content).await;
                }

                all_scores.push(scores);
            }
            Err(OsuError::NotFound) => {
                let content = format!("User `{name}` was not found");

                return data.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        }
    }

    drop(scores_futs);

    // Check if different names that both belong to the same user were given
    {
        let unique: HashSet<_> = users.iter().map(CommonUser::id).collect();

        if unique.len() == 1 {
            let content = "Give at least two different users";

            return data.error(&ctx, content).await;
        }
    }

    // Process users and their top scores for tracking
    for scores in all_scores.iter_mut() {
        process_osu_tracking(&ctx, scores, None).await;
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

    let mut scores_per_map: Vec<SmallVec<[CommonScoreEntry; 3]>> = all_scores
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

            let mut scores: SmallVec<[CommonScoreEntry; 3]> =
                scores.into_iter().map(CommonScoreEntry::new).collect();

            // Calculate the index of the pp ordered by their values
            if (scores[0].pp - scores[1].pp).abs() <= f32::EPSILON {
                match scores[1].score.score.cmp(&scores[0].score.score) {
                    Ordering::Less => scores[1].pos += 1,
                    Ordering::Equal => {
                        match scores[0].score.created_at.cmp(&scores[1].score.created_at) {
                            Ordering::Less => scores[1].pos += 1,
                            Ordering::Equal => {}
                            Ordering::Greater => scores[0].pos += 1,
                        }
                    }
                    Ordering::Greater => scores[0].pos += 1,
                }
            } else if scores[0].pp > scores[1].pp {
                scores[1].pos += 1;
            } else {
                scores[0].pos += 1;
            }

            if scores.len() == 3 {
                if (scores[0].pp - scores[2].pp).abs() <= f32::EPSILON {
                    match scores[2].score.score.cmp(&scores[0].score.score) {
                        Ordering::Less => scores[2].pos += 1,
                        Ordering::Equal => {
                            match scores[0].score.created_at.cmp(&scores[2].score.created_at) {
                                Ordering::Less => scores[2].pos += 1,
                                Ordering::Equal => {}
                                Ordering::Greater => scores[0].pos += 1,
                            }
                        }
                        Ordering::Greater => scores[0].pos += 1,
                    }
                } else if scores[0].pp > scores[2].pp {
                    scores[2].pos += 1;
                } else {
                    scores[0].pos += 1;
                }

                if (scores[1].pp - scores[2].pp).abs() <= f32::EPSILON {
                    match scores[2].score.score.cmp(&scores[1].score.score) {
                        Ordering::Less => scores[2].pos += 1,
                        Ordering::Equal => {
                            match scores[1].score.created_at.cmp(&scores[2].score.created_at) {
                                Ordering::Less => scores[2].pos += 1,
                                Ordering::Equal => {}
                                Ordering::Greater => scores[1].pos += 1,
                            }
                        }
                        Ordering::Greater => scores[1].pos += 1,
                    }
                } else if scores[1].pp > scores[2].pp {
                    scores[2].pos += 1;
                } else {
                    scores[1].pos += 1;
                }
            }

            if scores[0].pos == 0 {
                users[0].first_count += 1;
            } else if scores[1].pos == 0 {
                users[1].first_count += 1;
            } else {
                users[2].first_count += 1;
            }

            scores
        })
        .collect();

    // Sort the maps by their score's avg pp values
    scores_per_map.sort_unstable_by(|s1, s2| {
        let s1 = s1.iter().map(|entry| entry.pp).sum::<f32>() / s1.len() as f32;
        let s2 = s2.iter().map(|entry| entry.pp).sum::<f32>() / s2.len() as f32;

        s2.partial_cmp(&s1).unwrap_or(Ordering::Equal)
    });

    // Accumulate all necessary data
    let mut content = String::with_capacity(16);
    let len = users.len();
    let mut iter = users.iter().map(CommonUser::name);

    if let Some(first) = iter.next() {
        let last = iter.next_back();
        let _ = write!(content, "`{first}`");

        for name in iter {
            let _ = write!(content, ", `{name}`");
        }

        if let Some(name) = last {
            if len > 2 {
                content.push(',');
            }

            let _ = write!(content, " and `{name}`");
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
    let urls = users.iter().map(CommonUser::avatar_url);
    let thumbnail_fut = get_combined_thumbnail(&ctx, urls, users.len() as u32);

    let data_fut = async {
        let limit = scores_per_map.len().min(10);

        CommonEmbed::new(&users, &scores_per_map[..limit], 0)
    };

    let (thumbnail_result, embed_data) = tokio::join!(thumbnail_fut, data_fut);

    let thumbnail = match thumbnail_result {
        Ok(thumbnail) => Some(thumbnail),
        Err(why) => {
            let report = Report::new(why).wrap_err("failed to combine avatars");
            warn!("{:?}", report);

            None
        }
    };

    // Creating the embed
    let embed = embed_data.into_builder().build();
    let mut builder = MessageBuilder::new().content(content).embed(embed);

    if let Some(bytes) = thumbnail.as_deref() {
        builder = builder.file("avatar_fuse.png", bytes);
    }

    let response_raw = data.create_message(&ctx, builder).await?;

    // Skip pagination if too few entries
    if scores_per_map.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = CommonPagination::new(response, users, scores_per_map);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
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
pub async fn common(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match TripleArgs::args(&ctx, &mut args, msg.author.id, Some(GameMode::STD)).await {
                Ok(Ok(common_args)) => {
                    let data = CommandData::Message { msg, args, num };

                    _common(ctx, data, common_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_compare(ctx, *command).await,
    }
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
pub async fn commonmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match TripleArgs::args(&ctx, &mut args, msg.author.id, Some(GameMode::MNA)).await {
                Ok(Ok(common_args)) => {
                    let data = CommandData::Message { msg, args, num };

                    _common(ctx, data, common_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_compare(ctx, *command).await,
    }
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
pub async fn commontaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match TripleArgs::args(&ctx, &mut args, msg.author.id, Some(GameMode::TKO)).await {
                Ok(Ok(common_args)) => {
                    let data = CommandData::Message { msg, args, num };

                    _common(ctx, data, common_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_compare(ctx, *command).await,
    }
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
pub async fn commonctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match TripleArgs::args(&ctx, &mut args, msg.author.id, Some(GameMode::CTB)).await {
                Ok(Ok(common_args)) => {
                    let data = CommandData::Message { msg, args, num };

                    _common(ctx, data, common_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_compare(ctx, *command).await,
    }
}

pub struct CommonScoreEntry {
    pub pos: usize,
    pub pp: f32,
    pub score: Score,
}

impl CommonScoreEntry {
    fn new(score: Score) -> Self {
        Self {
            pos: 0,
            pp: score.pp.unwrap_or_default(),
            score,
        }
    }
}

pub struct CommonUser {
    name: Username,
    avatar_url: String,
    user_id: u32,
    pub first_count: usize,
}

impl CommonUser {
    fn new(name: Username, avatar_url: String, user_id: u32) -> Self {
        Self {
            name,
            avatar_url,
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

    fn avatar_url(&self) -> &str {
        self.avatar_url.as_str()
    }
}
