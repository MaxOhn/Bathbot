use super::{ErrorType, GradeArg};
use crate::{
    embeds::{EmbedData, RecentEmbed},
    pagination::{Pagination, RecentPagination},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder, Name,
};

use futures::future::TryFutureExt;
use hashbrown::HashMap;
use rosu_v2::prelude::{
    GameMode, Grade, OsuError,
    RankStatus::{Approved, Loved, Qualified, Ranked},
};
use std::{borrow::Cow, sync::Arc};
use tokio::time::{sleep, Duration};

async fn _recentpages(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: RecentPagesArgs,
) -> BotResult<()> {
    let RecentPagesArgs {
        name,
        grade,
        mode,
        index,
    } = args;

    let author_id = data.author()?.id;

    let name = match name {
        Some(name) => name,
        None => match ctx.get_link(author_id.0) {
            Some(name) => name,
            None => return super::require_link(&ctx, &data).await,
        },
    };

    let num = index.unwrap_or(1).saturating_sub(1);

    // Retrieve the user and their recent scores
    let user_fut = super::request_user(&ctx, &name, Some(mode)).map_err(From::from);

    let scores_fut = ctx
        .osu()
        .user_scores(name.as_str())
        .recent()
        .mode(mode)
        .limit(100)
        .include_fails(grade.map_or(true, |g| g.include_fails()));

    let scores_fut = super::prepare_scores(&ctx, scores_fut);

    let (user, mut scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((_, scores)) if scores.is_empty() => {
            let content = format!(
                "No recent {}plays found for user `{}`",
                match mode {
                    GameMode::STD => "",
                    GameMode::TKO => "taiko ",
                    GameMode::CTB => "ctb ",
                    GameMode::MNA => "mania ",
                },
                name
            );

            return data.error(&ctx, content).await;
        }
        Ok(tuple) => tuple,
        Err(ErrorType::Osu(OsuError::NotFound)) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(ErrorType::Osu(why)) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
        Err(ErrorType::Bot(why)) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    if let Some(grades) = grade {
        scores.retain(|s| match grades {
            GradeArg::Single(grade) => s.grade.eq_letter(grade),
            GradeArg::Range { bot, top } => s.grade >= bot && s.grade <= top,
        });
    }

    if scores.is_empty() {
        let content = format!(
            "There are no scores with the specified grades in \
            `{name}`'{genitive} recent history",
            name = name,
            genitive = if name.ends_with('s') { "" } else { "s" }
        );

        return data.error(&ctx, content).await;
    }

    let score = match scores.get(num) {
        Some(score) => score,
        None => {
            let content = format!(
                "There {verb} only {num} score{plural} {prop}in `{name}`'{genitive} recent history.",
                verb = if scores.len() != 1 { "are" } else { "is" },
                num = scores.len(),
                plural = if scores.len() != 1 { "s" } else { "" },
                prop = if grade.is_some() {
                    "with the given properties "
                } else {
                    ""
                },
                name = name,
                genitive = if name.ends_with('s') { "" } else { "s" }
            );

            return data.error(&ctx, content).await;
        }
    };

    // Prepare retrieval of the map's global top 50 and the user's top 100
    let map = score.map.as_ref().unwrap();
    let map_id = map.map_id;

    let map_score_fut = async {
        match map.status {
            Ranked | Loved | Qualified | Approved => {
                let fut = ctx
                    .osu()
                    .beatmap_user_score(map_id, user.user_id)
                    .mode(mode);

                Some(fut.await)
            }
            _ => None,
        }
    };

    let best_fut = async {
        match map.status {
            Ranked => {
                let fut = ctx
                    .osu()
                    .user_scores(user.user_id)
                    .best()
                    .limit(100)
                    .mode(mode);

                Some(fut.await)
            }
            _ => None,
        }
    };

    // Retrieve and parse response
    let (map_score_result, best_result) = tokio::join!(map_score_fut, best_fut);
    let mut map_scores = HashMap::with_capacity(scores.len());

    match map_score_result {
        None | Some(Err(OsuError::NotFound)) => {}
        Some(Ok(score)) => {
            map_scores.insert(map_id, score);
        }
        Some(Err(why)) => unwind_error!(warn, why, "Error while getting global scores: {}"),
    }

    let mut best = match best_result {
        None => None,
        Some(Ok(scores)) => Some(scores),
        Some(Err(why)) => {
            unwind_error!(warn, why, "Error while getting top scores: {}");

            None
        }
    };

    // Accumulate all necessary data
    let tries = scores
        .iter()
        .skip_while(|s| s != &score)
        .take_while(|s| s.map.as_ref().unwrap().map_id == map_id && s.mods == score.mods)
        .count();

    let map_score = map_scores.get(&map_id);
    let data_fut = RecentEmbed::new(&user, score, best.as_deref(), map_score, true);

    let embed_data = match data_fut.await {
        Ok(data) => data,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let content = format!("Try #{}", tries);
    let embed = embed_data.as_builder().build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    let response = data.create_message(&ctx, builder).await?.model().await?;

    ctx.store_msg(response.id);

    // Process user and their top scores for tracking
    if let Some(ref mut scores) = best {
        process_tracking(&ctx, mode, scores, Some(&user)).await;
    }

    // Skip pagination if too few entries
    if scores.len() <= 1 {
        tokio::spawn(async move {
            sleep(Duration::from_secs(60)).await;

            if !ctx.remove_msg(response.id) {
                return;
            }

            let builder = embed_data.into_builder().build().into();

            if let Err(why) = response.update_message(&ctx, builder).await {
                unwind_error!(warn, why, "Error minimizing recent msg: {}");
            }
        });

        return Ok(());
    }

    // Pagination
    let pagination = RecentPagination::new(
        Arc::clone(&ctx),
        response,
        user,
        scores,
        num,
        best,
        map_scores,
        embed_data,
    );

    let owner = author_id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (recent): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Same as `recent` but with pagination & arguments")]
#[long_desc(
    "Display a user's most recent play.\n\
    To start with a previous recent score, you can add a number right after the command, \
    e.g. `rp42 badewanne3` to get the 42nd most recent score.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...` where you can provide \
    either a single grade or a grade *range*.\n\
    Ranges can be specified like\n\
    - `a..b` e.g. `C..SH` to only keep scores with grades between C and SH\n\
    - `a..` e.g. `C..` to only keep scores with grade C or higher\n\
    - `..b` e.g. `..C` to only keep scores that have at most grade C\n\
    Available grades are `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username] [pass=true/false] [grade=grade[..grade]]")]
#[example("badewanne3 pass=true", "grade=a", "whitecat grade=B..sh")]
#[aliases("rp")]
pub async fn recentpages(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentPagesArgs::args(&ctx, &mut args, GameMode::STD, num) {
                Ok(recent_args) => {
                    _recentpages(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { .. } => panic!(),
    }
}

#[command]
#[short_desc("Same as `rm` but with pagination & arguments")]
#[long_desc(
    "Display a user's most recent mania play.\n\
    To start with a previous recent score, you can add a number right after the command, \
    e.g. `rpm42 badewanne3` to get the 42nd most recent score.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...` where you can provide \
    either a single grade or a grade *range*.\n\
    Ranges can be specified like\n\
    - `a..b` e.g. `C..SH` to only keep scores with grades between C and SH\n\
    - `a..` e.g. `C..` to only keep scores with grade C or higher\n\
    - `..b` e.g. `..C` to only keep scores that have at most grade C\n\
    Available grades are `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username] [pass=true/false] [grade=grade[..grade]]")]
#[example(
    "badewanne3 pass=true grade=b",
    "badewanne3 grade=B..SS",
    "badewanne3 grade=a..sh"
)]
#[aliases("rpm")]
pub async fn recentpagesmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentPagesArgs::args(&ctx, &mut args, GameMode::MNA, num) {
                Ok(recent_args) => {
                    _recentpages(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { .. } => panic!(),
    }
}

#[command]
#[short_desc("Same as `rt` but with pagination & arguments")]
#[long_desc(
    "Display a user's most recent taiko play.\n\
    To start with a previous recent score, you can add a number right after the command, \
    e.g. `rpt42 badewanne3` to get the 42nd most recent score.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...` where you can provide \
    either a single grade or a grade *range*.\n\
    Ranges can be specified like\n\
    - `a..b` e.g. `C..SH` to only keep scores with grades between C and SH\n\
    - `a..` e.g. `C..` to only keep scores with grade C or higher\n\
    - `..b` e.g. `..C` to only keep scores that have at most grade C\n\
    Available grades are `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username] [pass=true/false] [grade=grade[..grade]]")]
#[example(
    "badewanne3 pass=true grade=b",
    "badewanne3 grade=B..SS",
    "badewanne3 grade=a..sh"
)]
#[aliases("rpt")]
pub async fn recentpagestaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentPagesArgs::args(&ctx, &mut args, GameMode::TKO, num) {
                Ok(recent_args) => {
                    _recentpages(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { .. } => panic!(),
    }
}

#[command]
#[short_desc("Same as `rc` but with pagination & arguments")]
#[long_desc(
    "Display a user's most recent ctb play.\n\
    To start with a previous recent score, you can add a number right after the command, \
    e.g. `rpc42 badewanne3` to get the 42nd most recent score.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...` where you can provide \
    either a single grade or a grade *range*.\n\
    Ranges can be specified like\n\
    - `a..b` e.g. `C..SH` to only keep scores with grades between C and SH\n\
    - `a..` e.g. `C..` to only keep scores with grade C or higher\n\
    - `..b` e.g. `..C` to only keep scores that have at most grade C\n\
    Available grades are `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username] [pass=true/false] [grade=grade[..grade]]")]
#[example(
    "badewanne3 pass=true grade=b",
    "badewanne3 grade=B..SS",
    "badewanne3 grade=a..sh"
)]
#[aliases("rpc")]
pub async fn recentpagesctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentPagesArgs::args(&ctx, &mut args, GameMode::CTB, num) {
                Ok(recent_args) => {
                    _recentpages(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { .. } => panic!(),
    }
}

struct RecentPagesArgs {
    name: Option<Name>,
    grade: Option<GradeArg>,
    mode: GameMode,
    index: Option<usize>,
}

impl RecentPagesArgs {
    const ERR_PARSE_GRADE: &'static str = "Failed to parse `grade`.\n\
        Must be either a single grade or two grades of the form `a..b` e.g. `C..S`.\n\
        Valid grades are: `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`";

    fn args(
        ctx: &Context,
        args: &mut Args,
        mode: GameMode,
        index: Option<usize>,
    ) -> Result<Self, Cow<'static, str>> {
        let mut name = None;
        let mut grade = None;
        let mut passes = None;

        for arg in args.take(3) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "pass" | "p" | "passes" => match value {
                        "true" | "1" => passes = Some(true),
                        "false" | "0" => passes = Some(false),
                        _ => {
                            let content =
                                "Failed to parse `pass`. Must be either `true` or `false`.";

                            return Err(content.into());
                        }
                    },
                    "fail" | "fails" | "f" => match value {
                        "true" | "1" => passes = Some(false),
                        "false" | "0" => passes = Some(true),
                        _ => {
                            let content =
                                "Failed to parse `fail`. Must be either `true` or `false`.";

                            return Err(content.into());
                        }
                    },
                    "grade" | "g" => match value.find("..") {
                        Some(idx) => {
                            let bot = &value[..idx];
                            let top = &value[idx + 2..];

                            let min = if bot.is_empty() {
                                Grade::XH
                            } else if let Ok(grade) = bot.parse() {
                                grade
                            } else {
                                return Err(Self::ERR_PARSE_GRADE.into());
                            };

                            let max = if top.is_empty() {
                                Grade::D
                            } else if let Ok(grade) = top.parse() {
                                grade
                            } else {
                                return Err(Self::ERR_PARSE_GRADE.into());
                            };

                            let bot = if min < max { min } else { max };
                            let top = if min > max { min } else { max };

                            grade = Some(GradeArg::Range { bot, top })
                        }
                        None => match value.parse().map(GradeArg::Single) {
                            Ok(grade_) => grade = Some(grade_),
                            Err(_) => return Err(Self::ERR_PARSE_GRADE.into()),
                        },
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `grade` or `pass`.",
                            key
                        );

                        return Err(content.into());
                    }
                }
            } else {
                name = Some(Args::try_link_name(ctx, arg)?);
            }
        }

        grade = match passes {
            Some(true) => match grade {
                Some(GradeArg::Single(Grade::F)) => None,
                Some(GradeArg::Single(_)) => grade,
                Some(GradeArg::Range { bot, top }) => match (bot, top) {
                    (Grade::F, Grade::F) => None,
                    (Grade::F, _) => Some(GradeArg::Range { bot: Grade::D, top }),
                    (_, Grade::F) => Some(GradeArg::Range {
                        bot: Grade::D,
                        top: bot,
                    }),
                    _ => Some(GradeArg::Range { bot, top }),
                },
                None => Some(GradeArg::Range {
                    bot: Grade::D,
                    top: Grade::XH,
                }),
            },
            Some(false) => Some(GradeArg::Single(Grade::F)),
            None => grade,
        };

        let args = Self {
            name,
            grade,
            mode,
            index,
        };

        Ok(args)
    }
}
