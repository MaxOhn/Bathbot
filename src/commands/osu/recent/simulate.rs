use super::RecentSimulateArgs;
use crate::{
    embeds::{EmbedData, SimulateEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, CommandData, CommandDataCompact, Context, MessageBuilder,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

pub(super) async fn _recentsimulate(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    mut args: RecentSimulateArgs,
) -> BotResult<()> {
    let name = match args.name.take() {
        Some(name) => name,
        None => match ctx.get_link(data.author()?.id.0) {
            Some(name) => name,
            None => return super::require_link(&ctx, &data).await,
        },
    };

    let limit = args.index.map_or(1, |n| n + (n == 0) as usize);

    if limit > 50 {
        let content = "Recent history goes only 50 scores back.";

        return data.error(&ctx, content).await;
    }

    let mode = args.mode;

    // Retrieve the recent score
    let scores_fut = ctx
        .osu()
        .user_scores(name.as_str())
        .recent()
        .mode(mode)
        .include_fails(true)
        .limit(limit);

    let mut score = match scores_fut.await {
        Ok(scores) if scores.is_empty() => {
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
        Ok(scores) if scores.len() < limit => {
            let content = format!(
                "There are only {} many scores in `{}`'{} recent history.",
                scores.len(),
                name,
                if name.ends_with('s') { "" } else { "s" }
            );

            return data.error(&ctx, content).await;
        }
        Ok(mut scores) => match scores.pop() {
            Some(mut score) => match super::prepare_score(&ctx, &mut score).await {
                Ok(_) => score,
                Err(why) => {
                    let _ = data.error(&ctx, OSU_API_ISSUE).await;

                    return Err(why.into());
                }
            },
            None => {
                let content = format!("No recent plays found for user `{}`", name);

                return data.error(&ctx, content).await;
            }
        },
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let map = score.map.take().unwrap();
    let mapset = score.mapset.take().unwrap();

    // Accumulate all necessary data
    let embed_data = match SimulateEmbed::new(Some(score), &map, &mapset, args).await {
        Ok(data) => data,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let embed = embed_data.as_builder().build();
    let builder = MessageBuilder::new()
        .content("Simulated score:")
        .embed(embed);
    let response = data.create_message(&ctx, builder).await?;

    // TODO
    // ctx.store_msg(response.id);

    // Store map in DB
    if let Err(why) = ctx.psql().insert_beatmap(&map).await {
        unwind_error!(
            warn,
            why,
            "Error while storing simulate recent map in DB: {}"
        )
    }

    let data: CommandDataCompact = data.into();

    // Set map on garbage collection list if unranked
    let gb = ctx.map_garbage_collector(&map);

    // Minimize embed after delay
    tokio::spawn(async move {
        gb.execute(&ctx).await;
        sleep(Duration::from_secs(45)).await;

        // TODO
        // if !ctx.remove_msg(response.id) {
        //     return;
        // }

        let builder = embed_data.into_builder().build().into();

        if let Err(why) = data.update_message(&ctx, builder, response).await {
            unwind_error!(warn, why, "Error minimizing simulaterecent msg: {}");
        }
    });

    Ok(())
}

#[command]
#[short_desc("Unchoke a user's most recent play")]
#[long_desc(
    "Unchoke a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `sr42 badewanne3` to get the 42nd most recent score."
)]
#[usage(
    "[username] [+mods] [acc=number] [combo=integer] [n300=integer] [n100=integer] [n50=integer] [misses=integer]"
)]
#[example("badewanne3 +hr acc=99.3 n300=1422 misses=1")]
#[aliases("sr")]
pub async fn simulaterecent(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentSimulateArgs::args(&ctx, &mut args, GameMode::STD, num) {
                Ok(recent_args) => {
                    _recentsimulate(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, command).await,
    }
}

#[command]
#[short_desc("Display a perfect play on a user's most recently played mania map")]
#[long_desc(
    "Display a perfect play on a user's most recently played mania map.\n\
    To get a previous recent map, you can add a number right after the command,\n\
    e.g. `srm42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods] [score=number]")]
#[example("badewanne3 +dt score=895000")]
#[aliases("srm")]
pub async fn simulaterecentmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentSimulateArgs::args(&ctx, &mut args, GameMode::MNA, num) {
                Ok(recent_args) => {
                    _recentsimulate(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, command).await,
    }
}

#[command]
#[short_desc("Unchoke a user's most recent taiko play")]
#[long_desc(
    "Unchoke a user's most recent taiko play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `srt42 badewanne3` to get the 42nd most recent score."
)]
#[usage(
    "[username] [+mods] [acc=number] [combo=integer] [n300=integer] [n100=integer] [misses=integer]"
)]
#[example("badewanne3 +hr acc=99.3 n300=1422 misses=1")]
#[aliases("srt")]
pub async fn simulaterecenttaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentSimulateArgs::args(&ctx, &mut args, GameMode::TKO, num) {
                Ok(recent_args) => {
                    _recentsimulate(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, command).await,
    }
}

#[command]
#[short_desc("Unchoke a user's most recent ctb play")]
#[long_desc(
    "Unchoke a user's most recent ctb play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `src42 badewanne3` to get the 42nd most recent score.\n\
    Note: n300 = #fruits ~ n100 = #droplets ~ n50 = #tiny droplets."
)]
#[usage(
    "[username] [+mods] [acc=number] [combo=integer] [n300=integer] [n100=integer] [n50=integer] [misses=integer]"
)]
#[example("badewanne3 +hr acc=99.3 n300=1422 misses=1")]
#[aliases("src")]
pub async fn simulaterecentctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentSimulateArgs::args(&ctx, &mut args, GameMode::CTB, num) {
                Ok(recent_args) => {
                    _recentsimulate(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, command).await,
    }
}
