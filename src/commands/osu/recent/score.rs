use super::GradeArg;
use crate::{
    database::UserConfig,
    embeds::{EmbedData, RecentEmbed},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder,
};

use rosu_v2::prelude::{
    GameMode, Grade, OsuError,
    RankStatus::{Approved, Loved, Qualified, Ranked},
};
use std::{borrow::Cow, sync::Arc};
use tokio::time::{sleep, Duration};
use twilight_model::id::UserId;

pub(super) async fn _recent(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: RecentArgs,
) -> BotResult<()> {
    let RecentArgs {
        config,
        index,
        grade,
    } = args;

    let name = match config.name {
        Some(ref name) => name.as_str(),
        None => return super::require_link(&ctx, &data).await,
    };

    let mode = config.mode.unwrap_or(GameMode::STD);

    // Retrieve the user and their recent scores
    let user_fut = super::request_user(&ctx, name, Some(mode));

    let scores_fut = ctx
        .osu()
        .user_scores(name)
        .recent()
        .mode(mode)
        .limit(100)
        .include_fails(grade.map_or(true, |g| g.include_fails()));

    let (mut user, mut scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((_, scores)) if scores.is_empty() => {
            let content = format!(
                "No recent {}plays found for user `{}`",
                match mode {
                    GameMode::STD => "",
                    GameMode::TKO => "taiko ",
                    GameMode::CTB => "ctb ",
                    GameMode::MNA => "mania ",
                },
                name,
            );

            return data.error(&ctx, content).await;
        }
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Overwrite default mode
    user.mode = mode;

    match grade {
        Some(GradeArg::Single(grade)) => scores.retain(|score| score.grade == grade),
        Some(GradeArg::Range { bot, top }) => {
            scores.retain(|score| (bot..=top).contains(&score.grade))
        }
        None => {}
    }

    let num = index.unwrap_or(1).saturating_sub(1);
    let mut iter = scores.iter_mut().skip(num);

    let (score, tries) = match iter.next() {
        Some(score) => match super::prepare_score(&ctx, score).await {
            Ok(_) => {
                let mods = score.mods;
                let map_id = map_id!(score).unwrap();

                let tries = 1 + iter
                    .take_while(|s| map_id!(s).unwrap() == map_id && s.mods == mods)
                    .count();

                (score, tries)
            }
            Err(why) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        },
        None => {
            let content = format!(
                "There {verb} only {num} score{plural} in `{name}`'{genitive} recent history.",
                verb = if scores.len() != 1 { "are" } else { "is" },
                num = scores.len(),
                plural = if scores.len() != 1 { "s" } else { "" },
                name = name,
                genitive = if name.ends_with('s') { "" } else { "s" }
            );

            return data.error(&ctx, content).await;
        }
    };

    let map = score.map.as_ref().unwrap();

    // Prepare retrieval of the the user's top 50 and score position on the map
    let map_score_fut = async {
        if score.grade != Grade::F && matches!(map.status, Ranked | Loved | Qualified | Approved) {
            let fut = ctx
                .osu()
                .beatmap_user_score(map.map_id, user.user_id)
                .mode(mode);

            Some(fut.await)
        } else {
            None
        }
    };

    let best_fut = async {
        if score.grade != Grade::F && map.status == Ranked {
            let fut = ctx
                .osu()
                .user_scores(user.user_id)
                .best()
                .limit(100)
                .mode(mode);

            Some(fut.await)
        } else {
            None
        }
    };

    // Retrieve and parse response
    let (map_score_result, best_result) = tokio::join!(map_score_fut, best_fut);

    let map_score = match map_score_result {
        None | Some(Err(OsuError::NotFound)) => None,
        Some(Ok(score)) => Some(score),
        Some(Err(why)) => {
            unwind_error!(warn, why, "Error while getting global scores: {}");

            None
        }
    };

    let mut best = match best_result {
        None => None,
        Some(Ok(scores)) => Some(scores),
        Some(Err(why)) => {
            unwind_error!(warn, why, "Error while getting top scores: {}");

            None
        }
    };

    let data_fut = RecentEmbed::new(&user, score, best.as_deref(), map_score.as_ref(), false);

    let embed_data = match data_fut.await {
        Ok(data) => data,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let content = format!("Try #{}", tries);

    // Only maximize if config allows it
    if config.embeds_maximized {
        let embed = embed_data.as_builder().build();
        let builder = MessageBuilder::new().content(content).embed(embed);
        let response_raw = data.create_message(&ctx, builder).await?;

        let response = response_raw.model().await?;
        ctx.store_msg(response.id);

        // Set map on garbage collection list if unranked
        let gb = ctx.map_garbage_collector(map);

        // * Note: Don't store maps in DB as their max combo isn't available

        // Process user and their top scores for tracking
        if let Some(ref mut scores) = best {
            if let Err(why) = ctx.psql().store_scores_maps(scores.iter()).await {
                unwind_error!(warn, why, "Error while storing best maps in DB: {}");
            }

            process_tracking(&ctx, mode, scores, Some(&user)).await;
        }

        // Wait for minimizing
        tokio::spawn(async move {
            gb.execute(&ctx).await;
            sleep(Duration::from_secs(45)).await;

            if !ctx.remove_msg(response.id) {
                return;
            }

            let embed = embed_data.into_builder().build();
            let builder = MessageBuilder::new().embed(embed);

            if let Err(why) = response.update_message(&ctx, builder).await {
                unwind_error!(warn, why, "Error minimizing recent msg: {}");
            }
        });
    } else {
        let embed = embed_data.into_builder().build();

        let builder = MessageBuilder::new().content(content).embed(embed);
        data.create_message(&ctx, builder).await?;

        // Set map on garbage collection list if unranked
        let gb = ctx.map_garbage_collector(map);
        gb.execute(&ctx).await;

        // * Note: Don't store maps in DB as their max combo isn't available

        // Process user and their top scores for tracking
        if let Some(ref mut scores) = best {
            if let Err(why) = ctx.psql().store_scores_maps(scores.iter()).await {
                unwind_error!(warn, why, "Error while storing best maps in DB: {}");
            }

            process_tracking(&ctx, mode, scores, Some(&user)).await;
        }
    }

    Ok(())
}

#[command]
#[short_desc("Display a user's most recent play")]
#[long_desc(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `r42 badewanne3` to get the 42nd most recent score.\n\
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
#[aliases("r", "rs")]
pub async fn recent(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(recent_args.config.mode(GameMode::STD));

                    _recent(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display a user's most recent mania play")]
#[long_desc(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rm42 badewanne3` to get the 42nd most recent score.\n\
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
#[aliases("rm")]
pub async fn recentmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::MNA);

                    _recent(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display a user's most recent taiko play")]
#[long_desc(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rt42 badewanne3` to get the 42nd most recent score.\n\
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
#[aliases("rt")]
pub async fn recenttaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::TKO);

                    _recent(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display a user's most recent ctb play")]
#[long_desc(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rc42 badewanne3` to get the 42nd most recent score.\n\
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
#[aliases("rc")]
pub async fn recentctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::CTB);

                    _recent(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

pub(super) struct RecentArgs {
    pub config: UserConfig,
    pub index: Option<usize>,
    pub grade: Option<GradeArg>,
}

impl RecentArgs {
    const ERR_PARSE_GRADE: &'static str = "Failed to parse `grade`.\n\
        Must be either a single grade or two grades of the form `a..b` e.g. `C..S`.\n\
        Valid grades are: `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`";

    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
        index: Option<usize>,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut config = ctx.user_config(author_id).await?;
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

                            return Ok(Err(content.into()));
                        }
                    },
                    "fail" | "fails" | "f" => match value {
                        "true" | "1" => passes = Some(false),
                        "false" | "0" => passes = Some(true),
                        _ => {
                            let content =
                                "Failed to parse `fail`. Must be either `true` or `false`.";

                            return Ok(Err(content.into()));
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
                                return Ok(Err(Self::ERR_PARSE_GRADE.into()));
                            };

                            let max = if top.is_empty() {
                                Grade::D
                            } else if let Ok(grade) = top.parse() {
                                grade
                            } else {
                                return Ok(Err(Self::ERR_PARSE_GRADE.into()));
                            };

                            let bot = if min < max { min } else { max };
                            let top = if min > max { min } else { max };

                            grade = Some(GradeArg::Range { bot, top })
                        }
                        None => match value.parse().map(GradeArg::Single) {
                            Ok(grade_) => grade = Some(grade_),
                            Err(_) => return Ok(Err(Self::ERR_PARSE_GRADE.into())),
                        },
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `grade` or `pass`.",
                            key
                        );

                        return Ok(Err(content.into()));
                    }
                }
            } else {
                match Args::check_user_mention(ctx, arg).await? {
                    Ok(name) => config.name = Some(name),
                    Err(content) => return Ok(Err(content.into())),
                }
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
            config,
            index,
            grade,
        };

        Ok(Ok(args))
    }
}
