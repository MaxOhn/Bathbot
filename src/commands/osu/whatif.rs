use crate::{
    commands::{MyCommand, MyCommandOption},
    custom_client::RankParam,
    database::UserConfig,
    embeds::{EmbedData, WhatIfEmbed},
    tracking::process_tracking,
    util::{
        constants::{
            common_literals::{DISCORD, MODE, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        ApplicationCommandExt, InteractionExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, Error,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::{borrow::Cow, sync::Arc};
use twilight_model::{
    application::interaction::{application_command::CommandDataOption, ApplicationCommand},
    id::UserId,
};

use super::{option_discord, option_mode, option_name};

async fn _whatif(ctx: Arc<Context>, data: CommandData<'_>, args: WhatIfArgs) -> BotResult<()> {
    let WhatIfArgs { config, pp } = args;

    let name = match config.osu_username {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let mode = config.mode.unwrap_or(GameMode::STD);

    if pp < 0.0 {
        return data.error(&ctx, "The pp number must be non-negative").await;
    } else if pp > (i64::MAX / 1024) as f32 {
        return data.error(&ctx, "Number too large").await;
    }

    // Retrieve the user and their top scores
    let user_fut = super::request_user(&ctx, &name, Some(mode));

    let scores_fut = ctx
        .osu()
        .user_scores(name.as_str())
        .best()
        .mode(mode)
        .limit(100);

    let (mut user, mut scores) = match tokio::try_join!(user_fut, scores_fut) {
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

    // Process user and their top scores for tracking
    process_tracking(&ctx, mode, &mut scores, Some(&user)).await;

    let whatif_data = if scores.is_empty() {
        let rank_result = ctx
            .clients
            .custom
            .get_rank_data(mode, RankParam::Pp(pp))
            .await;

        let rank = match rank_result {
            Ok(rank_pp) => Some(rank_pp.rank),
            Err(why) => {
                unwind_error!(warn, why, "Error while getting rank pp: {}");

                None
            }
        };

        WhatIfData::NoScores { rank }
    } else if pp < scores.last().and_then(|s| s.pp).unwrap_or(0.0) {
        WhatIfData::NonTop100
    } else {
        let actual: f32 = scores
            .iter()
            .filter_map(|score| score.weight)
            .map(|weight| weight.pp)
            .sum();

        let bonus_pp = user.statistics.as_ref().unwrap().pp - actual;
        let mut potential = 0.0;
        let mut used = false;
        let mut new_pos = scores.len();
        let mut factor = 1.0;

        let pp_iter = scores
            .iter()
            .take(scores.len() - 1)
            .filter_map(|score| score.pp)
            .enumerate();

        for (i, pp_value) in pp_iter {
            if !used && pp_value < pp {
                used = true;
                potential += pp * factor;
                factor *= 0.95;
                new_pos = i + 1;
            }

            potential += pp_value * factor;
            factor *= 0.95;
        }

        if !used {
            potential += pp * factor;
        };

        let new_pp = potential;
        let max_pp = scores.first().and_then(|s| s.pp).unwrap_or(0.0);

        let rank_result = ctx
            .clients
            .custom
            .get_rank_data(mode, RankParam::Pp(new_pp + bonus_pp))
            .await;

        let rank = match rank_result {
            Ok(rank_pp) => Some(rank_pp.rank),
            Err(why) => {
                unwind_error!(warn, why, "Error while getting rank pp: {}");
                None
            }
        };

        WhatIfData::Top100 {
            bonus_pp,
            new_pp,
            new_pos,
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
        rank: Option<u32>,
    },
    Top100 {
        bonus_pp: f32,
        new_pp: f32,
        new_pos: usize,
        max_pp: f32,
        rank: Option<u32>,
    },
}

struct WhatIfArgs {
    config: UserConfig,
    pp: f32,
}

const WHATIF: &str = "whatif";

impl WhatIfArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
    ) -> BotResult<Result<Self, &'static str>> {
        let mut config = ctx.user_config(author_id).await?;
        let mut pp = None;

        for arg in args.take(2) {
            match arg.parse() {
                Ok(num) => pp = Some(num),
                Err(_) => match Args::check_user_mention(ctx, arg).await? {
                    Ok(name) => config.osu_username = Some(name),
                    Err(content) => return Ok(Err(content)),
                },
            }
        }

        let pp = match pp {
            Some(pp) => pp,
            None => return Ok(Err("You need to provide a decimal number")),
        };

        Ok(Ok(Self { config, pp }))
    }

    async fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut pp = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    MODE => config.mode = parse_mode_option!(value, "whatif"),
                    NAME => config.osu_username = Some(value.into()),
                    DISCORD => config.osu_username = parse_discord_option!(ctx, value, "whatif"),
                    "pp" => match value.parse() {
                        Ok(num) => pp = Some(num),
                        Err(_) => {
                            let content = "Failed to parse `pp`. Must be a number.";

                            return Ok(Err(content.into()));
                        }
                    },
                    _ => bail_cmd_option!(WHATIF, string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!(WHATIF, integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!(WHATIF, boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!(WHATIF, subcommand, name)
                }
            }
        }

        let args = Self {
            pp: pp.ok_or(Error::InvalidCommandOptions)?,
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
    // TODO: Number variant
    let pp = MyCommandOption::builder("pp", "Specify a pp amount").string(Vec::new(), true);
    let mode = option_mode();
    let name = option_name();
    let discord = option_discord();

    let description = "Display the impact of a new X pp score for a user";

    MyCommand::new(WHATIF, description).options(vec![pp, mode, name, discord])
}
