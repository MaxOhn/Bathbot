use super::{prepare_scores, request_user, ErrorType};
use crate::{
    arguments::{Args, GradeArg, NameGradePassArgs},
    embeds::{EmbedData, RecentEmbed},
    pagination::{Pagination, RecentPagination},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, Context,
};

use futures::future::TryFutureExt;
use hashbrown::HashMap;
use rosu_v2::prelude::{
    GameMode, OsuError,
    RankStatus::{Approved, Loved, Qualified, Ranked},
};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use twilight_model::channel::Message;

async fn recent_pages_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
    num: Option<usize>,
) -> BotResult<()> {
    let args = match NameGradePassArgs::new(&ctx, args) {
        Ok(args) => args,
        Err(err_msg) => return msg.error(&ctx, err_msg).await,
    };

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    let num = num.unwrap_or(1).saturating_sub(1);

    // Retrieve the user and their recent scores
    let user_fut = request_user(&ctx, &name, Some(mode)).map_err(From::from);

    let scores_fut = ctx
        .osu()
        .user_scores(name.as_str())
        .recent()
        .mode(mode)
        .limit(50)
        .include_fails(true);

    let scores_fut = prepare_scores(&ctx, scores_fut);

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

            return msg.error(&ctx, content).await;
        }
        Ok(tuple) => tuple,
        Err(ErrorType::Osu(OsuError::NotFound)) => {
            let content = format!("User `{}` was not found", name);

            return msg.error(&ctx, content).await;
        }
        Err(ErrorType::Osu(why)) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
        Err(ErrorType::Bot(why)) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    if let Some(grades) = args.grade {
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

        return msg.error(&ctx, content).await;
    }

    let score = match scores.get(num) {
        Some(score) => score,
        None => {
            let content = format!(
                "There {verb} only {num} score{plural} {prop}in `{name}`'{genitive} recent history.",
                verb = if scores.len() != 1 { "are" } else { "is" },
                num = scores.len(),
                plural = if scores.len() != 1 { "s" } else { "" },
                prop = if args.grade.is_some() {
                    "with the given properties "
                } else {
                    ""
                },
                name = name,
                genitive = if name.ends_with('s') { "" } else { "s" }
            );

            return msg.error(&ctx, content).await;
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
                    .limit(50)
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

    let data = match data_fut.await {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let embed = data.build().build()?;

    let response = ctx
        .http
        .create_message(msg.channel_id)
        .content(format!("Try #{}", tries))?
        .embed(embed)?
        .await?;

    ctx.store_msg(response.id);

    // Process user and their top scores for tracking
    if let Some(ref mut scores) = best {
        process_tracking(&ctx, mode, scores, Some(&user)).await;
    }

    // Skip pagination if too few entries
    if scores.len() <= 1 {
        response.reaction_delete(&ctx, msg.author.id);

        tokio::spawn(async move {
            sleep(Duration::from_secs(60)).await;

            if !ctx.remove_msg(response.id) {
                return;
            }

            let update_fut = ctx
                .http
                .update_message(response.channel_id, response.id)
                .embed(data.minimize().build().unwrap())
                .unwrap();

            if let Err(why) = update_fut.await {
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
        data,
    );

    let owner = msg.author.id;

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
    If the argument `-pass` is given, all fails will be filtered out.\n\
    Also, a grade can be specified with `-grade`, either followed by \
    grading letters `SS`, `S`, `A`, `B`, `C`, or `D`, or followed by a \
    range of the form `x..y` with `x` and `y` being a grading letter.\n\
    If a grade is specified, all scores that don't fit will be filtered out."
)]
#[usage("[username] [-pass] [-grade grade[..grade]]")]
#[example("badewanne3 -pass", "badewanne3 -grade B", "badewanne3 -grade A..SS")]
#[aliases("rp")]
pub async fn recentpages(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
    num: Option<usize>,
) -> BotResult<()> {
    recent_pages_main(GameMode::STD, ctx, msg, args, num).await
}

#[command]
#[short_desc("Same as `rm` but with pagination & arguments")]
#[long_desc(
    "Display a user's most recent mania play.\n\
    To start with a previous recent score, you can add a number right after the command, \
    e.g. `rpm42 badewanne3` to get the 42nd most recent score.\n\
    If the argument `-pass` is given, all fails will be filtered out.\n\
    Also, a grade can be specified with `-grade`, either followed by \
    grading letters `SS`, `S`, `A`, `B`, `C`, or `D`, or followed by a \
    range of the form `x..y` with `x` and `y` being a grading letter.\n\
    If a grade is specified, all scores that don't fit will be filtered out."
)]
#[usage("[username] [-pass] [-grade grade[..grade]]")]
#[example("badewanne3 -pass", "badewanne3 -grade B", "badewanne3 -grade A..SS")]
#[aliases("rpm")]
pub async fn recentpagesmania(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
    num: Option<usize>,
) -> BotResult<()> {
    recent_pages_main(GameMode::MNA, ctx, msg, args, num).await
}

#[command]
#[short_desc("Same as `rt` but with pagination & arguments")]
#[long_desc(
    "Display a user's most recent taiko play.\n\
    To start with a previous recent score, you can add a number right after the command, \
    e.g. `rpt42 badewanne3` to get the 42nd most recent score.\n\
    If the argument `-pass` is given, all fails will be filtered out.\n\
    Also, a grade can be specified with `-grade`, either followed by \
    grading letters `SS`, `S`, `A`, `B`, `C`, or `D`, or followed by a \
    range of the form `x..y` with `x` and `y` being a grading letter.\n\
    If a grade is specified, all scores that don't fit will be filtered out."
)]
#[usage("[username] [-pass] [-grade grade[..grade]]")]
#[example("badewanne3 -pass", "badewanne3 -grade B", "badewanne3 -grade A..SS")]
#[aliases("rpt")]
pub async fn recentpagestaiko(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
    num: Option<usize>,
) -> BotResult<()> {
    recent_pages_main(GameMode::TKO, ctx, msg, args, num).await
}

#[command]
#[short_desc("Same as `rc` but with pagination & arguments")]
#[long_desc(
    "Display a user's most recent ctb play.\n\
    To start with a previous recent score, you can add a number right after the command, \
    e.g. `rpc42 badewanne3` to get the 42nd most recent score.\n\
    If the argument `-pass` is given, all fails will be filtered out.\n\
    Also, a grade can be specified with `-grade`, either followed by \
    grading letters `SS`, `S`, `A`, `B`, `C`, or `D`, or followed by a \
    range of the form `x..y` with `x` and `y` being a grading letter.\n\
    If a grade is specified, all scores that don't fit will be filtered out."
)]
#[usage("[username] [-pass] [-grade grade[..grade]]")]
#[example("badewanne3 -pass", "badewanne3 -grade B", "badewanne3 -grade A..SS")]
#[aliases("rpc")]
pub async fn recentpagesctb(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
    num: Option<usize>,
) -> BotResult<()> {
    recent_pages_main(GameMode::CTB, ctx, msg, args, num).await
}
