use crate::{
    arguments::{Args, NameDashPArgs},
    embeds::{EmbedData, RecentEmbed},
    tracking::process_tracking,
    unwind_error,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher, MessageExt,
    },
    BotResult, Context,
};

use rosu::model::{
    ApprovalStatus::{Approved, Loved, Qualified, Ranked},
    GameMode, Score,
};
use std::{collections::HashMap, sync::Arc};
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

    let num = num.unwrap_or(1).saturating_sub(1);

    // Retrieve the user and their recent scores
    let user_fut = ctx.osu().user(name.as_str()).mode(mode);
    let scores_fut = ctx.osu().recent_scores(name.as_str()).mode(mode).limit(50);

    let (user, scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((None, _)) => {
            let content = format!("User `{}` was not found", name);

            return msg.error(&ctx, content).await;
        }
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
        Ok((Some(user), scores)) => (user, scores),
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

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

    let map_id = score.beatmap_id.unwrap();
    let mut store_in_db = false;

    let map = match ctx.psql().get_beatmap(map_id).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(Some(map)) => {
                store_in_db = true;

                map
            }
            Ok(None) => {
                let content = format!("The API returned no beatmap for map id {}", map_id);

                return msg.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        },
    };

    // Prepare retrieval of the map's global top 50 and the user's top 100
    let global_fut = async {
        match map.approval_status {
            Ranked | Loved | Qualified | Approved => {
                Some(map.get_global_leaderboard(ctx.osu()).limit(50).await)
            }
            _ => None,
        }
    };

    let best_fut = async {
        match map.approval_status {
            Ranked => Some(user.get_top_scores(ctx.osu()).limit(100).mode(mode).await),
            _ => None,
        }
    };

    // Retrieve and parse response
    let (globals_result, best_result) = tokio::join!(global_fut, best_fut);

    let globals: Option<Vec<Score>> = match globals_result {
        None => None,
        Some(Ok(scores)) => Some(scores),
        Some(Err(why)) => {
            unwind_error!(warn, why, "Error while getting global scores: {}");

            None
        }
    };

    let best: Option<Vec<Score>> = match best_result {
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
        .take_while(|s| s.beatmap_id.unwrap() == map_id && s.enabled_mods == score.enabled_mods)
        .count();

    let data_fut = RecentEmbed::new(&user, score, &map, best.as_deref(), globals.as_deref());

    let data = match data_fut.await {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let mut embed = data.build().build()?;

    // Funny numeral
    if mode == GameMode::STD {
        for idx in 1..=5 {
            embed.fields[idx].value =
                matcher::highlight_funny_numeral(&embed.fields[idx].value).into_owned();
        }
    }

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

    // Store map in DB
    if store_in_db {
        match ctx.psql().insert_beatmap(&map).await {
            Ok(true) => info!("Added map {} to DB", map.beatmap_id),
            Ok(false) => {}
            Err(why) => unwind_error!(
                warn,
                why,
                "Error while storing map {} in DB: {}",
                map.beatmap_id
            ),
        }
    }

    // Process user and their top scores for tracking
    if let Some(ref scores) = best {
        let mut maps = HashMap::new();
        maps.insert(map.beatmap_id, map);

        process_tracking(&ctx, mode, scores, Some(&user), &mut maps).await;
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
