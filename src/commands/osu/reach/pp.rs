use crate::{
    custom_client::RankParam,
    embeds::{EmbedData, PPMissingEmbed},
    tracking::process_tracking,
    util::{constants::OSU_API_ISSUE, MessageExt},
    Args, BotResult, CommandData, Context, Error, Name,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;
use twilight_model::application::interaction::application_command::CommandDataOption;

pub(super) async fn _pp(ctx: Arc<Context>, data: CommandData<'_>, args: PpArgs) -> BotResult<()> {
    let PpArgs { name, mode, pp } = args;

    let name = match name {
        Some(name) => name,
        None => match ctx.get_link(data.author()?.id.0) {
            Some(name) => name,
            None => return super::require_link(&ctx, &data).await,
        },
    };

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

    let user = match user_result {
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
            match PpArgs::args(&ctx, &mut args, GameMode::STD) {
                Ok(whatif_args) => {
                    _pp(ctx, CommandData::Message { msg, args, num }, whatif_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, command).await,
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
            match PpArgs::args(&ctx, &mut args, GameMode::MNA) {
                Ok(whatif_args) => {
                    _pp(ctx, CommandData::Message { msg, args, num }, whatif_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, command).await,
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
            match PpArgs::args(&ctx, &mut args, GameMode::TKO) {
                Ok(whatif_args) => {
                    _pp(ctx, CommandData::Message { msg, args, num }, whatif_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, command).await,
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
            match PpArgs::args(&ctx, &mut args, GameMode::CTB) {
                Ok(whatif_args) => {
                    _pp(ctx, CommandData::Message { msg, args, num }, whatif_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, command).await,
    }
}

pub(super) struct PpArgs {
    pub name: Option<Name>,
    pub mode: GameMode,
    pub pp: f32,
}

impl PpArgs {
    fn args(ctx: &Context, args: &mut Args, mode: GameMode) -> Result<Self, &'static str> {
        let mut name = None;
        let mut pp = None;

        for arg in args.take(2) {
            match arg.parse() {
                Ok(num) => pp = Some(num),
                Err(_) => name = Some(Args::try_link_name(ctx, arg)?),
            }
        }

        let pp = pp.ok_or("You need to provide a decimal number")?;

        Ok(Self { name, pp, mode })
    }

    pub(super) fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
    ) -> BotResult<Result<Self, String>> {
        let mut username = None;
        let mut mode = None;
        let mut pp = None;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "mode" => mode = parse_mode_option!(value, "pp"),
                    "name" => username = Some(value.into()),
                    "discord" => username = parse_discord_option!(ctx, value, "pp"),
                    _ => bail_cmd_option!("pp", string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("pp", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("pp", boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("pp", subcommand, name)
                }
            }
        }

        let args = Self {
            pp: pp.ok_or(Error::InvalidCommandOptions)?,
            name: username,
            mode: mode.unwrap_or(GameMode::STD),
        };

        Ok(Ok(args))
    }
}
