use std::sync::Arc;

use eyre::Report;
use rosu_v2::prelude::{GameMode, OsuError};
use tokio::time::{sleep, Duration};
use twilight_model::{
    application::interaction::{
        application_command::{CommandDataOption, CommandOptionValue},
        ApplicationCommand,
    },
    id::UserId,
};

use crate::{
    commands::{check_user_mention, parse_discord, parse_mode_option, DoubleResultCow},
    database::UserConfig,
    embeds::{EmbedData, SimulateEmbed},
    error::Error,
    util::{
        constants::{
            common_literals::{
                ACC, ACCURACY, COMBO, DISCORD, INDEX, MISSES, MODS, MODS_PARSE_FAIL, NAME, SCORE,
            },
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        matcher,
        osu::ModSelection,
        InteractionExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder,
};

pub(super) async fn _recentsimulate(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: RecentSimulateArgs,
) -> BotResult<()> {
    let name = match args.config.username() {
        Some(name) => name.as_str(),
        None => return super::require_link(&ctx, &data).await,
    };

    let mode = args.config.mode.unwrap_or(GameMode::STD);
    let limit = args.index.map_or(1, |n| n + (n == 0) as usize);

    if limit > 100 {
        let content = "Recent history goes only 100 scores back.";

        return data.error(&ctx, content).await;
    }

    // Retrieve the recent score
    let scores_fut = ctx
        .osu()
        .user_scores(name)
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
    let maximize = args.config.embeds_maximized();

    // Accumulate all necessary data
    let embed_data = match SimulateEmbed::new(Some(score), &map, &mapset, args.into()).await {
        Ok(data) => data,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let content = "Simulated score:";

    // Only maximize if config allows it
    if maximize {
        let embed = embed_data.as_builder().build();
        let builder = MessageBuilder::new().content(content).embed(embed);
        let response = data.create_message(&ctx, builder).await?.model().await?;

        ctx.store_msg(response.id);

        // Store map in DB
        if let Err(err) = ctx.psql().insert_beatmap(&map).await {
            warn!("{:?}", Report::new(err));
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

            let embed = embed_data.into_builder().build();
            let builder = MessageBuilder::new().content(content).embed(embed);

            if let Err(why) = response.update_message(&ctx, builder).await {
                let report = Report::new(why).wrap_err("failed to minimize message");
                warn!("{:?}", report);
            }
        });
    } else {
        let embed = embed_data.into_builder().build();
        let builder = MessageBuilder::new().content(content).embed(embed);
        data.create_message(&ctx, builder).await?;

        // Store map in DB
        if let Err(err) = ctx.psql().insert_beatmap(&map).await {
            warn!("{:?}", Report::new(err));
        }

        // Set map on garbage collection list if unranked
        ctx.map_garbage_collector(&map).execute(&ctx).await;
    }

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
            match RecentSimulateArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode.get_or_insert(GameMode::STD);

                    _recentsimulate(ctx, CommandData::Message { msg, args, num }, recent_args).await
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
            match RecentSimulateArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::MNA);

                    _recentsimulate(ctx, CommandData::Message { msg, args, num }, recent_args).await
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
            match RecentSimulateArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::TKO);

                    _recentsimulate(ctx, CommandData::Message { msg, args, num }, recent_args).await
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
            match RecentSimulateArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::CTB);

                    _recentsimulate(ctx, CommandData::Message { msg, args, num }, recent_args).await
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

pub struct RecentSimulateArgs {
    pub(super) config: UserConfig,
    pub(super) index: Option<usize>,
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
        return Ok(Err(format!(
            concat!("Failed to parse `{}`. Must be ", $ty, "."),
            $key
        )
        .into()))
    };
}

impl RecentSimulateArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
        index: Option<usize>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(author_id).await?;
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
                    MISSES | "miss" | "m" => match value.parse() {
                        Ok(value) => misses = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    ACC | "a" | ACCURACY => match value.parse() {
                        Ok(value) => acc = Some(value),
                        Err(_) => parse_fail!(key, "a number"),
                    },
                    COMBO | "c" => match value.parse() {
                        Ok(value) => combo = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    SCORE | "s" => match value.parse() {
                        Ok(value) => score = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    MODS => match value.parse() {
                        Ok(m) => mods = Some(ModSelection::Exact(m)),
                        Err(_) => return Ok(Err(MODS_PARSE_FAIL.into())),
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `n300`, `n100`, `n50`, \
                            `misses`, `acc`, `combo`, and `score`.",
                            key
                        );

                        return Ok(Err(content.into()));
                    }
                }
            } else if let Some(mods_) = matcher::get_mods(arg) {
                mods.replace(mods_);
            } else {
                match check_user_mention(ctx, arg).await? {
                    Ok(osu) => config.osu = Some(osu),
                    Err(content) => return Ok(Err(content)),
                }
            }
        }

        let args = Self {
            config,
            index,
            mods,
            n300,
            n100,
            n50,
            misses,
            acc,
            combo,
            score,
        };

        Ok(Ok(args))
    }

    pub(super) async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        mut options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut mods = None;
        let mut index = None;
        let mut n300 = None;
        let mut n100 = None;
        let mut n50 = None;
        let mut misses = None;
        let mut acc = None;
        let mut combo = None;
        let mut score = None;

        let option = options.pop().ok_or(Error::InvalidCommandOptions)?;

        match option.value {
            CommandOptionValue::SubCommand(options) => {
                for option in options {
                    match option.value {
                        CommandOptionValue::String(value) => match option.name.as_str() {
                            NAME => config.osu = Some(value.into()),
                            MODS => match matcher::get_mods(&value) {
                                Some(mods_) => mods = Some(mods_),
                                None => match value.parse() {
                                    Ok(mods_) => mods = Some(ModSelection::Exact(mods_)),
                                    Err(_) => return Ok(Err(MODS_PARSE_FAIL.into())),
                                },
                            },
                            // TODO: Remove
                            ACC => match value.parse::<f32>() {
                                Ok(number) => acc = Some(number.clamp(0.0, 100.0)),
                                Err(_) => {
                                    let content = "Failed to parse accuracy. \
                                        Be sure you specify a valid number between 0 and 100.";

                                    return Ok(Err(content.into()));
                                }
                            },
                            _ => return Err(Error::InvalidCommandOptions),
                        },
                        CommandOptionValue::Integer(value) => match option.name.as_str() {
                            INDEX => index = Some(value.max(1).min(50) as usize),
                            "n300" | "fruits" => n300 = Some(value.max(0) as usize),
                            "n100" | "droplets" => n100 = Some(value.max(0) as usize),
                            "n50" | "tiny_droplets" => n50 = Some(value.max(0) as usize),
                            MISSES => misses = Some(value.max(0) as usize),
                            COMBO => combo = Some(value.max(0) as usize),
                            SCORE => score = Some(value.max(0) as u32),
                            _ => return Err(Error::InvalidCommandOptions),
                        },
                        CommandOptionValue::Number(value) => {
                            let number = (option.name == ACC)
                                .then(|| value.0 as f32)
                                .ok_or(Error::InvalidCommandOptions)?;

                            acc = Some(number.max(0.0).min(100.0));
                        }
                        CommandOptionValue::User(value) => match option.name.as_str() {
                            DISCORD => match parse_discord(ctx, value).await? {
                                Ok(osu) => config.osu = Some(osu),
                                Err(content) => return Ok(Err(content)),
                            },
                            _ => return Err(Error::InvalidCommandOptions),
                        },
                        _ => return Err(Error::InvalidCommandOptions),
                    }
                }

                config.mode = parse_mode_option(&option.name);
            }
            _ => return Err(Error::InvalidCommandOptions),
        }

        let args = Self {
            config,
            mods,
            index,
            n300,
            n100,
            n50,
            misses,
            acc,
            combo,
            score,
        };

        Ok(Ok(args))
    }
}
