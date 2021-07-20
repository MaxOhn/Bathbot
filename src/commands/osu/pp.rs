use super::request_user;
use crate::{
    arguments::{Args, NameFloatArgs},
    custom_client::RankParam,
    embeds::{EmbedData, PPMissingEmbed},
    tracking::process_tracking,
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;
use twilight_model::channel::Message;

async fn pp_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = match NameFloatArgs::new(&ctx, args) {
        Ok(args) => args,
        Err(err_msg) => return msg.error(&ctx, err_msg).await,
    };

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    let pp = args.float;

    if pp < 0.0 {
        return msg.error(&ctx, "The pp number must be non-negative").await;
    } else if pp > (i64::MAX / 1024) as f32 {
        return msg.error(&ctx, "Number too large").await;
    }

    // Retrieve the user and their top scores
    let user_fut = request_user(&ctx, &name, Some(mode));
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

            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let mut scores = match scores_result {
        Ok(scores) => scores,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

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
    let data = PPMissingEmbed::new(user, scores, pp, rank);

    // Creating the embed
    let embed = data.into_builder().build();
    msg.build_response(&ctx, |m| m.embed(embed)).await?;

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
pub async fn pp(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    pp_main(GameMode::STD, ctx, msg, args).await
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
pub async fn ppmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    pp_main(GameMode::MNA, ctx, msg, args).await
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
pub async fn pptaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    pp_main(GameMode::TKO, ctx, msg, args).await
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
pub async fn ppctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    pp_main(GameMode::CTB, ctx, msg, args).await
}
