use crate::{
    arguments::{Args, NameFloatArgs},
    custom_client::RankParam,
    embeds::{EmbedData, PPMissingEmbed},
    tracking::process_tracking,
    unwind_error,
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use rosu::model::GameMode;
use std::{collections::HashMap, sync::Arc};
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
        let content = "The pp number must be non-negative";
        return msg.error(&ctx, content).await;
    } else if pp > (i64::MAX / 1024) as f32 {
        let content = "Number too large";
        return msg.error(&ctx, content).await;
    }

    // Retrieve the user and their top scores
    let user_fut = ctx.osu().user(name.as_str()).mode(mode);
    let scores_fut = ctx.osu().top_scores(name.as_str()).mode(mode).limit(100);
    let rank_fut = ctx.clients.custom.get_rank_data(mode, RankParam::Pp(pp));

    let (user_result, scores_result, rank_result) = tokio::join!(user_fut, scores_fut, rank_fut);

    let user = match user_result {
        Ok(Some(user)) => user,
        Ok(None) => {
            let content = format!("User `{}` was not found", name);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };

    let scores = match scores_result {
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
    let mut maps = HashMap::new();
    process_tracking(&ctx, mode, &scores, Some(&user), &mut maps).await;

    // Accumulate all necessary data
    let data = PPMissingEmbed::new(user, scores, pp, rank);

    // Creating the embed
    let embed = data.build().build()?;
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
