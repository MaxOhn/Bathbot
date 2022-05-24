use std::{cmp::Ordering, fmt::Write, iter, sync::Arc};

use chrono::{DateTime, Utc};
use command_macros::command;
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::{
    prelude::{GameMode, OsuError, Score, Username},
    OsuResult,
};

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
    let args = CompareTop::args(None, args);

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
    let args = CompareTop::args(Some(GameModeOption::Mania), args);

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
    let args = CompareTop::args(Some(GameModeOption::Taiko), args);

    top(ctx, msg.into(), args).await
}

#[command]
#[desc("Compare maps of two players' top100s")]
#[help("Compare the ctb users' top 100 and check which maps appear in each top list")]
#[usage("[name1] [name2]")]
#[example("badewanne3 \"nathan on osu\"")]
#[aliases("commonc", "commoncatch")]
#[group(Catch)]
async fn prefix_commonctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    let args = CompareTop::args(Some(GameModeOption::Catch), args);

    top(ctx, msg.into(), args).await
}

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

    let mode = match args.mode {
        Some(mode) => mode.into(),
        None => match ctx.user_config(orig.user_id()?).await {
            Ok(config) => config.mode.unwrap_or(GameMode::STD),
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let fut1 = get_scores_(&ctx, &name1, mode);
    let fut2 = get_scores_(&ctx, &name2, mode);

    let (mut scores1, mut scores2) = match tokio::join!(fut1, fut2) {
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

    let user1 = if let Some(score) = scores1.first() {
        let user_id = score.user_id;
        let avatar_url = score.user.as_ref().unwrap().avatar_url.clone();
        name1.make_ascii_lowercase();

        CommonUser::new(name1, avatar_url, user_id)
    } else {
        let content = format!("User `{name1}` has no {mode} top scores");

        return orig.error(&ctx, content).await;
    };

    let user2 = if let Some(score) = scores2.first() {
        let user_id = score.user_id;
        let avatar_url = score.user.as_ref().unwrap().avatar_url.clone();
        name2.make_ascii_lowercase();

        CommonUser::new(name2, avatar_url, user_id)
    } else {
        let content = format!("User `{name2}` has no {mode} top scores");

        return orig.error(&ctx, content).await;
    };

    // Check if different names that both belong to the same user were given
    if user1.id() == user2.id() {
        let content = "You must two different users";

        return orig.error(&ctx, content).await;
    }

    // Process users and their top scores for tracking
    let tracking_fut1 = process_osu_tracking(&ctx, &mut scores1, None);
    let tracking_fut2 = process_osu_tracking(&ctx, &mut scores2, None);
    tokio::join!(tracking_fut1, tracking_fut2);

    let indices: HashMap<_, _> = scores2
        .iter()
        .enumerate()
        .map(|(i, score)| (score.map.as_ref().unwrap().map_id, i))
        .collect();

    let mut wins = [0, 0];

    let maps: HashMap<_, _> = scores1
        .into_iter()
        .filter_map(|mut score1| {
            let map = score1.map.take()?;
            let mapset = score1.mapset.take()?;

            let score1 = CommonScore::from(&score1);

            let idx = indices.get(&map.map_id)?;
            let score2 = CommonScore::from(&scores2[*idx]);

            match score1.cmp(&score2) {
                Ordering::Less => wins[1] += 1,
                Ordering::Equal => {}
                Ordering::Greater => wins[0] += 1,
            }

            Some((map.map_id, ([score1, score2], map, mapset)))
        })
        .collect();

    // Sort the maps by their score's avg pp values
    let mut map_pps: Vec<_> = maps
        .iter()
        .map(|(map_id, ([a, b], ..))| (*map_id, a.pp + b.pp))
        .collect();

    map_pps.sort_unstable_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(Ordering::Equal));

    // Accumulate all necessary data
    let mut content = format!("`{}` and `{}` ", user1.name, user2.name);

    let amount_common = maps.len();

    if amount_common == 0 {
        content.push_str("have no common scores");
    } else {
        let _ = write!(
            content,
            "have {amount_common} common beatmap{} in their top 100",
            if amount_common > 1 { "s" } else { "" }
        );
    }

    // Create the combined profile pictures
    let urls = iter::once(user1.avatar_url()).chain(iter::once(user2.avatar_url()));
    let thumbnail_result = get_combined_thumbnail(&ctx, urls, 2, None).await;
    let limit = maps.len().min(10);
    let embed_data = CommonEmbed::new(&user1.name, &user2.name, &map_pps[..limit], &maps, wins, 0);

    let thumbnail = match thumbnail_result {
        Ok(thumbnail) => Some(thumbnail),
        Err(err) => {
            let report = Report::new(err).wrap_err("failed to combine avatars");
            warn!("{:?}", report);

            None
        }
    };

    // Creating the embed
    let embed = embed_data.build();
    let mut builder = MessageBuilder::new().content(content).embed(embed);

    if let Some(bytes) = thumbnail {
        builder = builder.attachment("avatar_fuse.png", bytes);
    }

    let response_raw = orig.create_message(&ctx, &builder).await?;

    // Skip pagination if too few entries
    if maps.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = CommonPagination::new(response, user1.name, user2.name, maps, map_pps, wins);
    pagination.start(ctx, orig.user_id()?, 60);

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

#[derive(PartialEq)]
pub struct CommonScore {
    pub pp: f32,
    score: u32,
    created_at: DateTime<Utc>,
}

impl Eq for CommonScore {}

impl From<&Score> for CommonScore {
    fn from(score: &Score) -> Self {
        Self {
            pp: score.pp.unwrap_or(0.0),
            score: score.score,
            created_at: score.created_at,
        }
    }
}

impl Ord for CommonScore {
    fn cmp(&self, other: &Self) -> Ordering {
        self.pp
            .partial_cmp(&other.pp)
            .unwrap_or(Ordering::Equal)
            .then_with(|| self.score.cmp(&other.score))
            .then_with(|| other.created_at.cmp(&self.created_at))
    }
}

impl PartialOrd for CommonScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
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

    fn avatar_url(&self) -> &str {
        self.avatar_url.as_str()
    }
}

impl<'m> CompareTop<'m> {
    fn args(mode: Option<GameModeOption>, args: Args<'m>) -> Self {
        let mut args_ = CompareTop {
            mode,
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
