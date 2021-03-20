use super::{prepare_scores, ErrorType};
use crate::{
    arguments::{Args, NameDashPArgs},
    embeds::{EmbedData, RecentEmbed},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, Context,
};

use futures::future::TryFutureExt;
use rosu_v2::prelude::{
    GameMode, OsuError,
    RankStatus::{Approved, Loved, Qualified, Ranked},
    Score,
};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use twilight_model::channel::Message;

async fn recent_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
    num: Option<usize>,
) -> BotResult<()> {
    let args = NameDashPArgs::new(&ctx, args);

    if args.has_dash_p {
        let prefix = ctx.config_first_prefix(msg.guild_id);

        let content = format!(
            "`{prefix}recent{mode} -p`? \
            Try putting the number right after the command, e.g. `{prefix}recent{mode}42`.\n\
            Alternatively you can checkout the `recentpages{mode}` command.",
            mode = match mode {
                GameMode::STD => "",
                GameMode::MNA => "mania",
                GameMode::TKO => "taiko",
                GameMode::CTB => "ctb",
            },
            prefix = prefix
        );

        return msg.error(&ctx, content).await;
    }

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    // Retrieve the user and their recent scores
    let user_fut = ctx.osu().user(name.as_str()).mode(mode).map_err(From::from);

    let scores_fut = ctx
        .osu()
        .user_scores(&name)
        .recent()
        .mode(mode)
        .limit(50)
        .include_fails(true);

    let scores_fut = prepare_scores(&ctx, scores_fut);

    let (user, scores) = match tokio::try_join!(user_fut, scores_fut) {
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

            return msg.error(&ctx, content).await;
        }
        Ok((user, scores)) => (user, scores),
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

    let num = num.unwrap_or(1).saturating_sub(1);

    let score = match scores.get(num) {
        Some(score) => score,
        None => {
            let content = format!(
                "There {verb} only {num} score{plural} in `{name}`'{genitive} recent history.",
                verb = if scores.len() != 1 { "are" } else { "is" },
                num = scores.len(),
                plural = if scores.len() != 1 { "s" } else { "" },
                name = name,
                genitive = if name.ends_with('s') { "" } else { "s" }
            );

            return msg.error(&ctx, content).await;
        }
    };

    let map = score.map.as_ref().unwrap();

    // Prepare retrieval of the map's global top 50 and the user's top 100
    let map_score_fut = async {
        match map.status {
            Ranked | Loved | Qualified | Approved => {
                let fut = ctx
                    .osu()
                    .beatmap_user_score(map.map_id, user.user_id)
                    .mode(mode);

                Some(fut.await)
            }
            _ => None,
        }
    };

    let best_fut = async {
        match map.status {
            Ranked => Some(
                ctx.osu()
                    .user_scores(user.user_id)
                    .best()
                    .limit(50)
                    .mode(mode)
                    .await,
            ),
            _ => None,
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

    let mut best: Option<Vec<Score>> = match best_result {
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
        .skip(num)
        .take_while(|s| s.map.as_ref().unwrap().map_id == map.map_id && s.mods == score.mods)
        .count();

    let data_fut = RecentEmbed::new(&user, score, best.as_deref(), map_score.as_ref(), false);

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

    response.reaction_delete(&ctx, msg.author.id);
    ctx.store_msg(response.id);

    // Set map on garbage collection list if unranked
    let gb = ctx.map_garbage_collector(&map);

    // Store maps in DB
    if let Err(why) = ctx.psql().store_scores_maps(scores.iter()).await {
        unwind_error!(warn, why, "Error while storing recent maps in DB: {}");
    }

    // Process user and their top scores for tracking
    if let Some(ref mut scores) = best {
        if let Err(why) = ctx.psql().store_scores_maps(scores.iter()).await {
            unwind_error!(warn, why, "Error while storing best maps in DB: {}");
        }

        process_tracking(&ctx, mode, scores).await;
    }

    // Wait for minimizing
    tokio::spawn(async move {
        gb.execute(&ctx).await;
        sleep(Duration::from_secs(45)).await;

        if !ctx.remove_msg(response.id) {
            return;
        }

        let embed = data.minimize().build().unwrap();

        let embed_update = ctx
            .http
            .update_message(response.channel_id, response.id)
            .embed(embed)
            .unwrap();

        if let Err(why) = embed_update.await {
            unwind_error!(warn, why, "Error minimizing recent msg: {}");
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display a user's most recent play")]
#[long_desc(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `r42 badewanne3` to get the 42nd most recent score."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("r", "rs")]
pub async fn recent(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
    num: Option<usize>,
) -> BotResult<()> {
    recent_main(GameMode::STD, ctx, msg, args, num).await
}

#[command]
#[short_desc("Display a user's most recent mania play")]
#[long_desc(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rm42 badewanne3` to get the 42nd most recent score."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rm")]
pub async fn recentmania(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
    num: Option<usize>,
) -> BotResult<()> {
    recent_main(GameMode::MNA, ctx, msg, args, num).await
}

#[command]
#[short_desc("Display a user's most recent taiko play")]
#[long_desc(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rt42 badewanne3` to get the 42nd most recent score."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rt")]
pub async fn recenttaiko(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
    num: Option<usize>,
) -> BotResult<()> {
    recent_main(GameMode::TKO, ctx, msg, args, num).await
}

#[command]
#[short_desc("Display a user's most recent ctb play")]
#[long_desc(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rc42 badewanne3` to get the 42nd most recent score."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rc")]
pub async fn recentctb(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
    num: Option<usize>,
) -> BotResult<()> {
    recent_main(GameMode::CTB, ctx, msg, args, num).await
}
