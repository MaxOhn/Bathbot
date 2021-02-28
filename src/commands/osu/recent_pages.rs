use crate::{
    arguments::{Args, GradeArg, NameGradePassArgs},
    embeds::{EmbedData, RecentEmbed},
    pagination::{Pagination, RecentPagination},
    tracking::process_tracking,
    unwind_error,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, Context,
};

use rosu::model::{
    ApprovalStatus::{Approved, Loved, Qualified, Ranked},
    GameMode,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
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
    let user_fut = ctx.osu().user(name.as_str()).mode(mode);
    let scores_fut = ctx.osu().recent_scores(name.as_str()).mode(mode).limit(50);

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
        Ok((None, _)) => {
            let content = format!("User `{}` was not found", name);

            return msg.error(&ctx, content).await;
        }
        Ok((Some(user), scores)) => (user, scores),
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
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

    // Get all relevant maps from the database
    let mut map_ids: HashSet<u32> = scores.iter().filter_map(|s| s.beatmap_id).collect();

    let mut maps = {
        let dedubed_ids: Vec<u32> = map_ids.iter().copied().collect();
        let map_result = ctx.psql().get_beatmaps(&dedubed_ids).await;

        match map_result {
            Ok(maps) => maps,
            Err(why) => {
                unwind_error!(warn, why, "Error while retrieving maps from DB: {}");

                HashMap::default()
            }
        }
    };

    // Memoize which maps are already in the DB
    map_ids.retain(|id| maps.contains_key(&id));

    // Retrieve the first map
    let map_id = score.beatmap_id.unwrap();

    #[allow(clippy::map_entry)]
    if !maps.contains_key(&map_id) {
        let map = match ctx.osu().beatmap().map_id(map_id).await {
            Ok(Some(map)) => map,
            Ok(None) => {
                let content = format!("The API returned no beatmap for map id {}", map_id);

                return msg.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        };

        maps.insert(map_id, map);
    }

    // Prepare retrieval of the map's global top 50 and the user's top 100
    let first_map = maps.get(&map_id).unwrap();

    let global_fut = async {
        match first_map.approval_status {
            Ranked | Loved | Qualified | Approved => {
                Some(first_map.get_global_leaderboard(ctx.osu()).limit(50).await)
            }
            _ => None,
        }
    };

    let best_fut = async {
        match first_map.approval_status {
            Ranked => Some(user.get_top_scores(ctx.osu()).limit(100).mode(mode).await),
            _ => None,
        }
    };

    let mut map_scores = HashMap::new();

    if let Ranked | Qualified | Approved | Loved = first_map.approval_status {
        let scores_fut = ctx
            .osu()
            .scores(first_map.beatmap_id)
            .user(user.user_id)
            .mode(mode);

        match scores_fut.await {
            Ok(scores) => {
                map_scores.insert(first_map.beatmap_id, scores);
            }
            Err(why) => unwind_error!(warn, why, "Error while requesting map scores: {}"),
        }
    }

    // Retrieve and parse response
    let (globals_result, best_result) = tokio::join!(global_fut, best_fut);
    let mut global = HashMap::with_capacity(scores.len());

    match globals_result {
        None => {}
        Some(Ok(scores)) => {
            global.insert(map_id, scores);
        }
        Some(Err(why)) => unwind_error!(warn, why, "Error while getting global scores: {}"),
    }

    let best = match best_result {
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
        .take_while(|s| s.beatmap_id.unwrap() == map_id && s.enabled_mods == score.enabled_mods)
        .count();

    let global_scores = global.get(&map_id).map(|global| global.as_slice());
    let first_map = maps.get(&map_id).unwrap();

    let data_fut = RecentEmbed::new(
        &user,
        score,
        first_map,
        best.as_deref(),
        global_scores,
        Some(&map_scores),
    );

    let data = match data_fut.await {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .content(format!("Try #{}", tries))?
        .embed(data.build().build()?)?
        .await?;

    ctx.store_msg(response.id);

    // Process user and their top scores for tracking
    if let Some(ref scores) = best {
        process_tracking(&ctx, mode, scores, Some(&user), &mut maps).await;
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
        maps,
        best,
        global,
        map_ids,
        data,
        map_scores,
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
