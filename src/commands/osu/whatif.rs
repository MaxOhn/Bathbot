use std::{cmp::Ordering, iter, sync::Arc};

use eyre::Report;
use rosu_v2::prelude::{GameMode, OsuError};
use twilight_model::{
    application::interaction::{application_command::CommandOptionValue, ApplicationCommand},
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::{
        check_user_mention, parse_discord, parse_mode_option, DoubleResultCow, MyCommand,
        MyCommandOption,
    },
    custom_client::RankParam,
    database::UserConfig,
    embeds::{EmbedData, WhatIfEmbed},
    tracking::process_osu_tracking,
    util::{
        constants::{
            common_literals::{DISCORD, MODE, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        ApplicationCommandExt, InteractionExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, Error,
};

use super::{get_user_and_scores, option_discord, option_mode, option_name, ScoreArgs, UserArgs};

async fn _whatif(ctx: Arc<Context>, data: CommandData<'_>, args: WhatIfArgs) -> BotResult<()> {
    let WhatIfArgs { config, pp, count } = args;
    let mode = config.mode.unwrap_or(GameMode::STD);

    let name = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    if pp < 0.0 {
        return data.error(&ctx, "The pp number must be non-negative").await;
    } else if pp > (i64::MAX / 1024) as f32 {
        return data.error(&ctx, "Number too large").await;
    }

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(name.as_str(), mode);
    let score_args = ScoreArgs::top(100);

    let (mut user, mut scores) = match get_user_and_scores(&ctx, user_args, &score_args).await {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Overwrite default mode
    user.mode = mode;

    // Process user and their top scores for tracking
    process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    let whatif_data = if scores.is_empty() {
        let pp = iter::repeat(pp)
            .enumerate()
            .take(count)
            .fold(0.0, |sum, (i, pp)| sum + pp * 0.95_f32.powi(i as i32));

        let rank_result = ctx
            .clients
            .custom
            .get_rank_data(mode, RankParam::Pp(pp))
            .await;

        let rank = match rank_result {
            Ok(rank_pp) => Some(rank_pp.rank),
            Err(why) => {
                let report = Report::new(why).wrap_err("error while getting rank pp");
                warn!("{report:?}");

                None
            }
        };

        WhatIfData::NoScores { count, rank }
    } else if pp < scores.last().and_then(|s| s.pp).unwrap_or(0.0) {
        WhatIfData::NonTop100
    } else {
        let actual: f32 = scores
            .iter()
            .filter_map(|score| score.weight)
            .map(|weight| weight.pp)
            .sum();

        let bonus_pp = user.statistics.as_ref().map_or(0.0, |stats| stats.pp) - actual;

        let idx = scores
            .iter()
            .filter_map(|s| s.pp)
            .position(|score_pp| score_pp < pp)
            .unwrap_or_else(|| scores.len() - 1);

        let mut pps = Vec::with_capacity(scores.len() + count);

        let pp_iter = scores
            .iter()
            .filter_map(|s| s.pp)
            .chain(iter::repeat(pp).take(count));

        pps.extend(pp_iter);
        pps.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));

        let new_pp = pps
            .iter()
            .enumerate()
            .fold(0.0, |sum, (i, pp)| sum + pp * 0.95_f32.powi(i as i32));

        let max_pp = scores.first().and_then(|s| s.pp).unwrap_or(0.0);

        let rank_fut = ctx
            .clients
            .custom
            .get_rank_data(mode, RankParam::Pp(new_pp + bonus_pp));

        let rank = match rank_fut.await {
            Ok(rank_pp) => Some(rank_pp.rank),
            Err(why) => {
                let report = Report::new(why).wrap_err("error while getting rank pp");
                warn!("{report:?}");

                None
            }
        };

        WhatIfData::Top100 {
            bonus_pp,
            count,
            new_pp,
            new_pos: idx + 1,
            max_pp,
            rank,
        }
    };

    // Sending the embed
    let builder = WhatIfEmbed::new(user, pp, whatif_data)
        .into_builder()
        .build()
        .into();

    data.create_message(&ctx, builder).await?;

    Ok(())
}

#[command]
#[short_desc("Display the impact of a new X pp score for a user")]
#[long_desc(
    "Calculate the gain in pp if the user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[aliases("wi")]
pub async fn whatif(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match WhatIfArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut whatif_args)) => {
                    whatif_args.config.mode.get_or_insert(GameMode::STD);

                    _whatif(ctx, CommandData::Message { msg, args, num }, whatif_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => slash_whatif(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display the impact of a new X pp score for a mania user")]
#[long_desc(
    "Calculate the gain in pp if the mania user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[aliases("wim")]
pub async fn whatifmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match WhatIfArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut whatif_args)) => {
                    whatif_args.config.mode = Some(GameMode::MNA);

                    _whatif(ctx, CommandData::Message { msg, args, num }, whatif_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => slash_whatif(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display the impact of a new X pp score for a taiko user")]
#[long_desc(
    "Calculate the gain in pp if the taiko user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[aliases("wit")]
pub async fn whatiftaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match WhatIfArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut whatif_args)) => {
                    whatif_args.config.mode = Some(GameMode::TKO);

                    _whatif(ctx, CommandData::Message { msg, args, num }, whatif_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => slash_whatif(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display the impact of a new X pp score for a ctb user")]
#[long_desc(
    "Calculate the gain in pp if the ctb user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[aliases("wic")]
pub async fn whatifctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match WhatIfArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut whatif_args)) => {
                    whatif_args.config.mode = Some(GameMode::CTB);

                    _whatif(ctx, CommandData::Message { msg, args, num }, whatif_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => slash_whatif(ctx, *command).await,
    }
}

pub enum WhatIfData {
    NonTop100,
    NoScores {
        count: usize,
        rank: Option<u32>,
    },
    Top100 {
        bonus_pp: f32,
        count: usize,
        new_pp: f32,
        new_pos: usize,
        max_pp: f32,
        rank: Option<u32>,
    },
}

impl WhatIfData {
    pub fn count(&self) -> usize {
        match self {
            WhatIfData::NonTop100 => 0,
            WhatIfData::NoScores { count, .. } => *count,
            WhatIfData::Top100 { count, .. } => *count,
        }
    }
}

struct WhatIfArgs {
    config: UserConfig,
    pp: f32,
    count: usize,
}

impl WhatIfArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: Id<UserMarker>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(author_id).await?;
        let mut pp = None;

        for arg in args.take(2) {
            match arg.parse() {
                Ok(num) => pp = Some(num),
                Err(_) => match check_user_mention(ctx, arg).await? {
                    Ok(osu) => config.osu = Some(osu),
                    Err(content) => return Ok(Err(content)),
                },
            }
        }

        let pp = match pp {
            Some(pp) => pp,
            None => return Ok(Err("You need to provide a decimal number".into())),
        };

        let count = 1;

        Ok(Ok(Self { config, pp, count }))
    }

    async fn slash(ctx: &Context, command: &mut ApplicationCommand) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut pp = None;
        let mut count = None;

        for option in command.yoink_options() {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    MODE => config.mode = parse_mode_option(&value),
                    NAME => config.osu = Some(value.into()),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Number(value) => match option.name.as_str() {
                    "pp" => pp = Some(value.0 as f32),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Integer(value) => match option.name.as_str() {
                    "count" => count = Some(value as usize),
                    _ => return Err(Error::InvalidCommandOptions),
                },
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

        let args = Self {
            pp: pp.ok_or(Error::InvalidCommandOptions)?,
            count: count.unwrap_or(1),
            config,
        };

        Ok(Ok(args))
    }
}

pub async fn slash_whatif(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match WhatIfArgs::slash(&ctx, &mut command).await? {
        Ok(args) => _whatif(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn define_whatif() -> MyCommand {
    let pp = MyCommandOption::builder("pp", "Specify a pp amount")
        .min_num(0.0)
        .number(Vec::new(), true);

    let mode = option_mode();
    let name = option_name();

    let count_description = "Specify how many times a score should be added, defaults to 1";

    let count = MyCommandOption::builder("count", count_description)
        .min_int(1)
        .max_int(1000)
        .integer(Vec::new(), false);

    let discord = option_discord();

    let description = "Display the impact of a new X pp score for a user";

    MyCommand::new("whatif", description).options(vec![pp, mode, name, count, discord])
}
