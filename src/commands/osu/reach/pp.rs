use std::sync::Arc;

use eyre::Report;
use rosu_v2::prelude::{GameMode, OsuError};
use twilight_model::{
    application::interaction::{
        application_command::{CommandDataOption, CommandOptionValue},
        ApplicationCommand,
    },
    id::UserId,
};

use crate::{
    commands::{
        check_user_mention,
        osu::{get_user_and_scores, ScoreArgs, UserArgs},
        parse_discord, parse_mode_option, DoubleResultCow,
    },
    custom_client::RankParam,
    database::UserConfig,
    embeds::{EmbedData, PPMissingEmbed},
    tracking::process_tracking,
    util::{
        constants::{
            common_literals::{DISCORD, MODE, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        InteractionExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, Error,
};

pub(super) async fn _pp(ctx: Arc<Context>, data: CommandData<'_>, args: PpArgs) -> BotResult<()> {
    let PpArgs { config, pp } = args;
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
    let user_scores_fut = get_user_and_scores(&ctx, user_args, &score_args);
    let rank_fut = ctx.clients.custom.get_rank_data(mode, RankParam::Pp(pp));

    let (user_scores_result, rank_result) = tokio::join!(user_scores_fut, rank_fut);

    let (mut user, mut scores) = match user_scores_result {
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

    let rank = match rank_result {
        Ok(rank_pp) => Some(rank_pp.rank as usize),
        Err(why) => {
            let report = Report::new(why).wrap_err("failed to get rank pp");
            warn!("{:?}", report);

            None
        }
    };

    // Process user and their top scores for tracking
    process_tracking(&ctx, &mut scores, Some(&user)).await;

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
    async fn args(ctx: &Context, args: &mut Args<'_>, author_id: UserId) -> DoubleResultCow<Self> {
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

        Ok(Ok(Self { config, pp }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut pp = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    MODE => config.mode = parse_mode_option(&value),
                    NAME => config.osu = Some(value.into()),
                    // TODO: Remove
                    "pp" => match value.parse::<f32>() {
                        Ok(number) => pp = Some(number.max(0.0)),
                        Err(_) => {
                            let content = "Failed to parse pp. \
                                Be sure you specify a valid number";

                            return Ok(Err(content.into()));
                        }
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Number(value) => {
                    let number = (option.name == "pp")
                        .then(|| value.0 as f32)
                        .ok_or(Error::InvalidCommandOptions)?;

                    pp = Some(number);
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

        let args = Self {
            pp: pp.ok_or(Error::InvalidCommandOptions)?,
            config,
        };

        Ok(Ok(args))
    }
}
