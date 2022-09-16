use std::sync::Arc;

use command_macros::command;
use eyre::{Report, Result};
use rosu_v2::prelude::{GameMode, GameMods, OsuError};
use tokio::time::{sleep, Duration};

use crate::{
    commands::{
        osu::{prepare_score, require_link},
        GameModeOption,
    },
    core::commands::{prefix::Args, CommandOrigin},
    database::EmbedsSize,
    embeds::{SimulateArgs, SimulateEmbed},
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher, ChannelExt, CowUtils, MessageExt,
    },
    Context,
};

use super::{
    RecentSimulate, RecentSimulateCatch, RecentSimulateMania, RecentSimulateOsu,
    RecentSimulateTaiko,
};

#[command]
#[desc("Unchoke a user's most recent play")]
#[help(
    "Unchoke a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `sr42 badewanne3` to get the 42nd most recent score."
)]
#[usage(
    "[username] [+mods] [acc=number] [combo=integer] [n300=integer] [n100=integer] [n50=integer] [misses=integer]"
)]
#[example("badewanne3 +hr acc=99.3 n300=1422 misses=1")]
#[alias("sr")]
#[group(Osu)]
async fn prefix_simulaterecent(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentSimulate::args(GameModeOption::Osu, args) {
        Ok(args) => simulate(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a perfect play on a user's most recently played mania map")]
#[help(
    "Display a perfect play on a user's most recently played mania map.\n\
    To get a previous recent map, you can add a number right after the command,\n\
    e.g. `srm42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods] [score=number]")]
#[example("badewanne3 +dt score=895000")]
#[alias("srm")]
#[group(Mania)]
async fn prefix_simulaterecentmania(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> Result<()> {
    match RecentSimulate::args(GameModeOption::Mania, args) {
        Ok(args) => simulate(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Unchoke a user's most recent taiko play")]
#[help(
    "Unchoke a user's most recent taiko play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `srt42 badewanne3` to get the 42nd most recent score."
)]
#[usage(
    "[username] [+mods] [acc=number] [combo=integer] [n300=integer] [n100=integer] [misses=integer]"
)]
#[example("badewanne3 +hr acc=99.3 n300=1422 misses=1")]
#[alias("srt")]
#[group(Taiko)]
async fn prefix_simulaterecenttaiko(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> Result<()> {
    match RecentSimulate::args(GameModeOption::Taiko, args) {
        Ok(args) => simulate(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Unchoke a user's most recent ctb play")]
#[help(
    "Unchoke a user's most recent ctb play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `src42 badewanne3` to get the 42nd most recent score.\n\
    Note: n300 = #fruits ~ n100 = #droplets ~ n50 = #tiny droplets."
)]
#[usage(
    "[username] [+mods] [acc=number] [combo=integer] [n300=integer] [n100=integer] [n50=integer] [misses=integer]"
)]
#[example("badewanne3 +hr acc=99.3 n300=1422 misses=1")]
#[aliases("src", "simulaterecentcatch")]
#[group(Catch)]
async fn prefix_simulaterecentctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentSimulate::args(GameModeOption::Catch, args) {
        Ok(args) => simulate(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

pub(super) async fn simulate(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: RecentSimulate<'_>,
) -> Result<()> {
    let owner = orig.user_id()?;

    let (name, index, args, mode) = match args {
        RecentSimulate::Osu(args) => {
            let name = username!(ctx, orig, args);
            let index = args.index;
            let args = SimulateArgs::try_from(args);

            (name, index, args, GameMode::Osu)
        }
        RecentSimulate::Taiko(args) => {
            let name = username!(ctx, orig, args);
            let index = args.index;
            let args = SimulateArgs::try_from(args);

            (name, index, args, GameMode::Taiko)
        }
        RecentSimulate::Catch(args) => {
            let name = username!(ctx, orig, args);
            let index = args.index;
            let args = SimulateArgs::try_from(args);

            (name, index, args, GameMode::Catch)
        }
        RecentSimulate::Mania(args) => {
            let name = username!(ctx, orig, args);
            let index = args.index;
            let args = SimulateArgs::try_from(args);

            (name, index, args, GameMode::Mania)
        }
    };

    let args = match args {
        Ok(args) => args,
        Err(content) => return orig.error(&ctx, content).await,
    };

    let limit = index.map_or(1, |n| n + (n == 0) as usize);

    if limit > 100 {
        let content = "Recent history goes only 100 scores back.";

        return orig.error(&ctx, content).await;
    }

    let config = match ctx.user_config(owner).await {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to get user config"));
        }
    };

    let name = match name {
        Some(name) => name,
        None => match config.username() {
            Some(name) => name.to_owned(),
            None => return require_link(&ctx, &orig).await,
        },
    };

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
                "No recent {}plays found for user `{name}`",
                match mode {
                    GameMode::Osu => "",
                    GameMode::Taiko => "taiko ",
                    GameMode::Catch => "ctb ",
                    GameMode::Mania => "mania ",
                }
            );

            return orig.error(&ctx, content).await;
        }
        Ok(scores) if scores.len() < limit => {
            let content = format!(
                "There are only {} many scores in `{name}`'{} recent history.",
                scores.len(),
                if name.ends_with('s') { "" } else { "s" }
            );

            return orig.error(&ctx, content).await;
        }
        Ok(mut scores) => match scores.pop() {
            Some(mut score) => match prepare_score(&ctx, &mut score).await {
                Ok(_) => score,
                Err(err) => {
                    let _ = orig.error(&ctx, OSU_API_ISSUE).await;

                    return Err(err.into());
                }
            },
            None => {
                let content = format!("No recent plays found for user `{name}`");

                return orig.error(&ctx, content).await;
            }
        },
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get scores");

            return Err(report);
        }
    };

    let map = score.map.take().unwrap();
    let mapset = score.mapset.take().unwrap();

    let embeds_size = match (config.score_size, orig.guild_id()) {
        (Some(size), _) => size,
        (None, Some(guild)) => ctx.guild_embeds_maximized(guild).await,
        (None, None) => EmbedsSize::default(),
    };

    // Accumulate all necessary data
    let embed_data = match SimulateEmbed::new(Some(score), &map, &mapset, args, &ctx).await {
        Ok(data) => data,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to create embed"));
        }
    };

    let content = "Simulated score:";

    // Only maximize if config allows it
    match embeds_size {
        EmbedsSize::AlwaysMinimized => {
            let embed = embed_data.into_minimized();
            let builder = MessageBuilder::new().content(content).embed(embed);
            orig.create_message(&ctx, &builder).await?;
        }
        EmbedsSize::InitialMaximized => {
            let embed = embed_data.as_maximized();
            let builder = MessageBuilder::new().content(content).embed(embed);
            let response = orig.create_message(&ctx, &builder).await?.model().await?;

            ctx.store_msg(response.id);
            let ctx = Arc::clone(&ctx);

            // Minimize embed after delay
            tokio::spawn(async move {
                sleep(Duration::from_secs(45)).await;

                if !ctx.remove_msg(response.id) {
                    return;
                }

                let embed = embed_data.into_minimized();
                let builder = MessageBuilder::new().content(content).embed(embed);

                if let Err(err) = response.update(&ctx, &builder).await {
                    let report = Report::new(err).wrap_err("Failed to minimize embed");
                    warn!("{report:?}");
                }
            });
        }
        EmbedsSize::AlwaysMaximized => {
            let embed = embed_data.as_maximized();
            let builder = MessageBuilder::new().content(content).embed(embed);
            orig.create_message(&ctx, &builder).await?;
        }
    }

    // Set map on garbage collection list if unranked
    ctx.map_garbage_collector(&map).execute(&ctx);

    Ok(())
}

macro_rules! parse_fail {
    ($key:ident, $ty:literal) => {
        return Err(format!(concat!("Failed to parse `{}`. Must be ", $ty, "."), $key).into())
    };
}

impl<'m> RecentSimulate<'m> {
    fn args(mode: GameModeOption, args: Args<'m>) -> Result<Self, String> {
        let mut name = None;
        let mut discord = None;
        let mut mods = None;
        let mut n300 = None;
        let mut n100 = None;
        let mut n50 = None;
        let mut misses = None;
        let mut acc = None;
        let mut combo = None;
        let mut score = None;
        let num = args.num;

        for arg in args.map(|arg| arg.cow_to_ascii_lowercase()) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

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
                    "acc" | "a" | "accuracy" => match value.parse::<f32>() {
                        Ok(value) => acc = Some(value.clamp(0.0, 100.0)),
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
                    "mods" => match value.parse::<GameMods>() {
                        Ok(_) => mods = Some(format!("+{value}!").into()),
                        Err(_) => return Err("Failed to parse mods. Be sure to specify a valid abbreviation e.g. `hdhr`.".to_owned()),
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{key}`.\n\
                            Available options are: `n300`, `n100`, `n50`, \
                            `misses`, `acc`, `combo`, and `score`."
                        );

                        return Err(content);
                    }
                }
            } else if matcher::get_mods(&arg).is_some() {
                mods = Some(arg);
            } else if let Some(id) = matcher::get_mention_user(&arg) {
                discord = Some(id);
            } else {
                name = Some(arg);
            }
        }

        let index = num.map(|n| n as usize);

        let args = match mode {
            GameModeOption::Osu => Self::Osu(RecentSimulateOsu {
                name,
                mods,
                index,
                n300,
                n100,
                n50,
                misses,
                acc,
                combo,
                discord,
            }),
            GameModeOption::Taiko => Self::Taiko(RecentSimulateTaiko {
                name,
                mods,
                index,
                n300,
                n100,
                misses,
                acc,
                combo,
                discord,
            }),
            GameModeOption::Catch => Self::Catch(RecentSimulateCatch {
                name,
                mods,
                index,
                fruits: n300,
                droplets: n100,
                tiny_droplets: n50,
                misses,
                acc,
                combo,
                discord,
            }),
            GameModeOption::Mania => Self::Mania(RecentSimulateMania {
                name,
                mods,
                index,
                score,
                discord,
            }),
        };

        Ok(args)
    }
}
