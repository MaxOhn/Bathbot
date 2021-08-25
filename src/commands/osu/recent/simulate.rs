use crate::{
    embeds::{EmbedData, SimulateEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher,
        osu::ModSelection,
        MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder, Name,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::{borrow::Cow, sync::Arc};
use tokio::time::{sleep, Duration};

pub(super) async fn _recentsimulate(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    mut args: RecentSimulateArgs,
) -> BotResult<()> {
    let author_id = data.author()?.id;

    let mode = match ctx.user_config(author_id).await {
        Ok(config) => config.mode(args.mode),
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let name = match args.name.take() {
        Some(name) => name,
        None => match ctx.get_link(author_id.0) {
            Some(name) => name,
            None => return super::require_link(&ctx, &data).await,
        },
    };

    let limit = args.index.map_or(1, |n| n + (n == 0) as usize);

    if limit > 50 {
        let content = "Recent history goes only 50 scores back.";

        return data.error(&ctx, content).await;
    }

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
    let embed_data = match SimulateEmbed::new(Some(score), &map, &mapset, args.into()).await {
        Ok(data) => data,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let embed = embed_data.as_builder().build();
    let content = "Simulated score:";
    let builder = MessageBuilder::new().content(content).embed(embed);
    let response = data.create_message(&ctx, builder).await?.model().await?;

    ctx.store_msg(response.id);

    // Store map in DB
    if let Err(why) = ctx.psql().insert_beatmap(&map).await {
        unwind_error!(
            warn,
            why,
            "Error while storing simulate recent map in DB: {}"
        )
    }

    // Set map on garbage collection list if unranked
    let gb = ctx.map_garbage_collector(&map);

    // Minimize embed after delay
    tokio::spawn(async move {
        gb.execute(&ctx).await;
        sleep(Duration::from_secs(45)).await;

        if !ctx.remove_msg(response.id) {
            return;
        }

        let builder = embed_data.into_builder().build().into();

        if let Err(why) = response.update_message(&ctx, builder).await {
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

pub struct RecentSimulateArgs {
    pub(super) name: Option<Name>,
    pub(super) index: Option<usize>,
    pub(super) mode: GameMode,
    pub mods: Option<ModSelection>,
    pub n300: Option<usize>,
    pub n100: Option<usize>,
    pub n50: Option<usize>,
    pub misses: Option<usize>,
    pub acc: Option<f32>,
    pub combo: Option<usize>,
    pub score: Option<u32>,
}

macro_rules! parse_fail {
    ($key:ident, $ty:literal) => {
        return Err(format!(concat!("Failed to parse `{}`. Must be ", $ty, "."), $key).into());
    };
}

impl RecentSimulateArgs {
    fn args(
        ctx: &Context,
        args: &mut Args,
        mode: GameMode,
        index: Option<usize>,
    ) -> Result<Self, Cow<'static, str>> {
        let mut name = None;
        let mut mods = None;
        let mut n300 = None;
        let mut n100 = None;
        let mut n50 = None;
        let mut misses = None;
        let mut acc = None;
        let mut combo = None;
        let mut score = None;

        for arg in args {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = &arg[idx + 1..];

                match key {
                    "n300" => match value.parse() {
                        Ok(value) => n300 = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    "n100" => match value.parse() {
                        Ok(value) => n100 = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    "n50" => match value.parse() {
                        Ok(value) => n50 = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    "misses" | "miss" | "m" => match value.parse() {
                        Ok(value) => misses = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    "acc" | "a" | "accuracy" => match value.parse() {
                        Ok(value) => acc = Some(value),
                        Err(_) => parse_fail!(key, "a number"),
                    },
                    "combo" | "c" => match value.parse() {
                        Ok(value) => combo = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    "score" | "s" => match value.parse() {
                        Ok(value) => score = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    "mods" => match value.parse() {
                        Ok(m) => mods = Some(ModSelection::Exact(m)),
                        Err(_) => parse_fail!(key, "a valid mod abbreviation"),
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `n300`, `n100`, `n50`, \
                            `misses`, `acc`, `combo`, and `score`.",
                            key
                        );

                        return Err(content.into());
                    }
                }
            } else if let Some(mods_) = matcher::get_mods(arg) {
                mods.replace(mods_);
            } else {
                name = Some(Args::try_link_name(ctx, arg)?);
            }
        }

        Ok(Self {
            name,
            index,
            mode,
            mods,
            n300,
            n100,
            n50,
            misses,
            acc,
            combo,
            score,
        })
    }
}
