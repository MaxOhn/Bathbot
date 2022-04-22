use std::{cmp::Ordering, fmt::Write, sync::Arc};

use command_macros::command;
use eyre::Report;
use hashbrown::HashSet;
use itertools::Itertools;
use rosu_v2::{
    prelude::{GameMode, OsuError, Score, Username},
    OsuResult,
};
use smallvec::SmallVec;

use crate::{
    commands::{
        osu::{get_scores, NameExtraction, ScoreArgs, UserArgs},
        GameModeOption,
    },
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{CommonEmbed, EmbedData},
    pagination::{CommonPagination, Pagination},
    tracking::process_osu_tracking,
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        get_combined_thumbnail, matcher,
    },
    BotResult, Context,
};

use super::{CompareTop, AT_LEAST_ONE};

#[command]
#[desc("Compare maps of two players' top100s")]
#[help("Compare the two users' top 100 and check which maps appear in each top list.")]
#[usage("[name1] [name2]")]
#[example("badewanne3 \"nathan on osu\"")]
#[group(Osu)]
async fn prefix_common(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    let args = CompareTop::args(GameModeOption::Osu, args);

    top(ctx, msg.into(), args).await
}

#[command]
#[desc("Compare maps of two players' top100s")]
#[help("Compare the mania users' top 100 and check which maps appear in each top list")]
#[usage("[name1] [name2]")]
#[example("badewanne3 \"nathan on osu\"")]
#[alias("commonm")]
#[group(Mania)]
async fn prefix_commonmania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    let args = CompareTop::args(GameModeOption::Mania, args);

    top(ctx, msg.into(), args).await
}

#[command]
#[desc("Compare maps of two players' top100s")]
#[help("Compare the taiko users' top 100 and check which maps appear in each top list")]
#[usage("[name1] [name2]")]
#[example("badewanne3 \"nathan on osu\"")]
#[alias("commont")]
#[group(Taiko)]
async fn prefix_commontaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    let args = CompareTop::args(GameModeOption::Taiko, args);

    top(ctx, msg.into(), args).await
}

#[command]
#[desc("Compare maps of two players' top100s")]
#[help("Compare the ctb users' top 100 and check which maps appear in each top list")]
#[usage("[name1] [name2]")]
#[example("badewanne3 \"nathan on osu\"")]
#[alias("commonc")]
#[group(Catch)]
async fn prefix_commonctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    let args = CompareTop::args(GameModeOption::Catch, args);

    top(ctx, msg.into(), args).await
}

macro_rules! user_id {
    ($scores:ident[$idx:literal]) => {
        $scores[$idx].user.as_ref().unwrap().user_id
    };
}

type CommonUsers = SmallVec<[CommonUser; 3]>;

async fn extract_name(ctx: &Context, args: &mut CompareTop<'_>) -> NameExtraction {
    if let Some(name) = args.name1.take().or_else(|| args.name2.take()) {
        NameExtraction::Name(name.as_ref().into())
    } else if let Some(discord) = args.discord1.take().or_else(|| args.discord2.take()) {
        match ctx.psql().get_user_osu(discord).await {
            Ok(Some(osu)) => NameExtraction::Name(osu.into_username()),
            Ok(None) => {
                NameExtraction::Content(format!("<@{discord}> is not linked to an osu!profile"))
            }
            Err(err) => NameExtraction::Err(err),
        }
    } else {
        NameExtraction::None
    }
}

pub(super) async fn top(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    mut args: CompareTop<'_>,
) -> BotResult<()> {
    let mut name1 = match extract_name(&ctx, &mut args).await {
        NameExtraction::Name(name) => name,
        NameExtraction::Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
        NameExtraction::Content(content) => return orig.error(&ctx, content).await,
        NameExtraction::None => return orig.error(&ctx, AT_LEAST_ONE).await,
    };

    let mut name2 = match extract_name(&ctx, &mut args).await {
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

    let mode = args.mode.map_or(GameMode::STD, GameMode::from);

    let fut1 = get_scores_(&ctx, &name1, mode);
    let fut2 = get_scores_(&ctx, &name2, mode);

    let (scores1, scores2) = match tokio::join!(fut1, fut2) {
        (Ok(scores1), Ok(scores2)) => (scores1, scores2),
        (Err(OsuError::NotFound), _) => {
            let content = format!("User `{name1}` was not found");

            return orig.error(&ctx, content).await;
        }
        (_, Err(OsuError::NotFound)) => {
            let content = format!("User `{name2}` was not found");

            return orig.error(&ctx, content).await;
        }
        (Err(err), _) | (_, Err(err)) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    // TODO: find nicer structure
    let mut all_scores = Vec::<Vec<_>>::with_capacity(2);
    let mut users = CommonUsers::with_capacity(2);

    let opt = scores1
        .first()
        .and_then(|s| Some((s.user_id, s.user.as_ref()?.avatar_url.clone())));

    if let Some((user_id, avatar_url)) = opt {
        name1.make_ascii_lowercase();

        users.push(CommonUser::new(name1, avatar_url, user_id));
    } else {
        let content = format!("User `{name1}` has no {mode} top scores");

        return orig.error(&ctx, content).await;
    }

    all_scores.push(scores1);

    let opt = scores2
        .first()
        .and_then(|s| Some((s.user_id, s.user.as_ref()?.avatar_url.clone())));

    if let Some((user_id, avatar_url)) = opt {
        name2.make_ascii_lowercase();

        users.push(CommonUser::new(name2, avatar_url, user_id));
    } else {
        let content = format!("User `{name2}` has no {mode} top scores");

        return orig.error(&ctx, content).await;
    }

    all_scores.push(scores2);

    // Check if different names that both belong to the same user were given
    if users[0].id() == users[1].id() {
        let content = "Give at least two different users";

        return orig.error(&ctx, content).await;
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
    let thumbnail_fut = get_combined_thumbnail(&ctx, urls, users.len() as u32, None);

    let data_fut = async {
        let limit = scores_per_map.len().min(10);

        CommonEmbed::new(&users, &scores_per_map[..limit], 0)
    };

    let (thumbnail_result, embed_data) = tokio::join!(thumbnail_fut, data_fut);

    let thumbnail = match thumbnail_result {
        Ok(thumbnail) => Some(thumbnail),
        Err(err) => {
            let report = Report::new(err).wrap_err("failed to combine avatars");
            warn!("{:?}", report);

            None
        }
    };

    // Creating the embed
    let embed = embed_data.into_builder().build();
    let mut builder = MessageBuilder::new().content(content).embed(embed);

    if let Some(bytes) = thumbnail {
        builder = builder.attachment("avatar_fuse.png", bytes);
    }

    let response_raw = orig.create_message(&ctx, &builder).await?;

    // Skip pagination if too few entries
    if scores_per_map.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = CommonPagination::new(response, users, scores_per_map);
    let owner = orig.user_id()?;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

async fn get_scores_(ctx: &Context, name: &str, mode: GameMode) -> OsuResult<Vec<Score>> {
    let mut user_args = UserArgs::new(name, mode);
    let score_args = ScoreArgs::top(100);
    let scores_fut = get_scores(ctx, &user_args, &score_args);

    if let Some(alt_name) = user_args.whitespaced_name() {
        match scores_fut.await {
            Ok(scores) => Ok(scores),
            Err(OsuError::NotFound) => {
                user_args.name = &alt_name;

                get_scores(ctx, &user_args, &score_args).await
            }
            Err(err) => Err(err),
        }
    } else {
        scores_fut.await
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

impl<'m> CompareTop<'m> {
    fn args(mode: GameModeOption, args: Args<'m>) -> Self {
        let mut args_ = CompareTop {
            mode: Some(mode),
            ..Default::default()
        };

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

        args_
    }
}
