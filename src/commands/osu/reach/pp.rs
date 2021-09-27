use crate::{
    custom_client::RankParam,
    database::UserConfig,
    embeds::{EmbedData, PPMissingEmbed},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    Args, BotResult, CommandData, Context, Error,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::{borrow::Cow, sync::Arc};
use twilight_model::{
    application::interaction::application_command::CommandDataOption, id::UserId,
};

pub(super) async fn _pp(ctx: Arc<Context>, data: CommandData<'_>, args: PpArgs) -> BotResult<()> {
    let PpArgs { config, pp } = args;

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

    let rank_fut = ctx.clients.custom.get_rank_data(mode, RankParam::Pp(pp));

    let (user_result, scores_result, rank_result) = tokio::join!(user_fut, scores_fut, rank_fut);

    let mut user = match user_result {
        Ok(user) => user,
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

    let mut scores = match scores_result {
        Ok(scores) => scores,
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let rank = match rank_result {
        Ok(rank_pp) => Some(rank_pp.rank as usize),
        Err(why) => {
            unwind_error!(warn, why, "Error while getting rank pp: {}");

            None
        }
    };

    // Process user and their top scores for tracking
    process_tracking(&ctx, mode, &mut scores, Some(&user)).await;

    // Accumulate all necessary data
    let embed_data = PPMissingEmbed::new(user, scores, pp, rank);

    // Creating the embed
    let builder = embed_data.into_builder().build().into();
    data.create_message(&ctx, builder).await?;

    Ok(())
}

#[command]
#[short_desc("How many pp are missing to reach the given amount?")]
#[long_desc(
    "Calculate what score a user is missing to \
     reach the given total pp amount"
)]
#[usage("[username] [number]")]
#[example("badewanne3 8000")]
pub async fn pp(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match PpArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut pp_args)) => {
                    pp_args.config.mode.get_or_insert(GameMode::STD);

                    _pp(ctx, CommandData::Message { msg, args, num }, pp_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, *command).await,
    }
}

#[command]
#[short_desc("How many pp are missing to reach the given amount?")]
#[long_desc(
    "Calculate what score a mania user is missing to \
     reach the given total pp amount"
)]
#[usage("[username] [number]")]
#[example("badewanne3 8000")]
#[aliases("ppm")]
pub async fn ppmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match PpArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut pp_args)) => {
                    pp_args.config.mode = Some(GameMode::MNA);

                    _pp(ctx, CommandData::Message { msg, args, num }, pp_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, *command).await,
    }
}

#[command]
#[short_desc("How many pp are missing to reach the given amount?")]
#[long_desc(
    "Calculate what score a taiko user is missing to \
     reach the given total pp amount"
)]
#[usage("[username] [number]")]
#[example("badewanne3 8000")]
#[aliases("ppt")]
pub async fn pptaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match PpArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut pp_args)) => {
                    pp_args.config.mode = Some(GameMode::TKO);

                    _pp(ctx, CommandData::Message { msg, args, num }, pp_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, *command).await,
    }
}

#[command]
#[short_desc("How many pp are missing to reach the given amount?")]
#[long_desc(
    "Calculate what score a ctb user is missing to \
     reach the given total pp amount"
)]
#[usage("[username] [number]")]
#[example("badewanne3 8000")]
#[aliases("ppc")]
pub async fn ppctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match PpArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut pp_args)) => {
                    pp_args.config.mode = Some(GameMode::CTB);

                    _pp(ctx, CommandData::Message { msg, args, num }, pp_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, *command).await,
    }
}

pub(super) struct PpArgs {
    pub config: UserConfig,
    pub pp: f32,
}

impl PpArgs {
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

    pub(super) async fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
        author_id: UserId,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut config = ctx.user_config(author_id).await?;
        let mut pp = None;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "mode" => config.mode = parse_mode_option!(value, "reach pp"),
                    "name" => config.osu_username = Some(value.into()),
                    "discord" => {
                        config.osu_username = parse_discord_option!(ctx, value, "reach pp")
                    }
                    "pp" => match value.parse() {
                        Ok(num) => pp = Some(num),
                        Err(_) => {
                            let content = "Failed to parse `pp`. Must be a number.";

                            return Ok(Err(content.into()));
                        }
                    },
                    _ => bail_cmd_option!("reach pp", string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("reach pp", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("reach pp", boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("reach pp", subcommand, name)
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
