use crate::{
    arguments::{Args, NameFloatArgs},
    embeds::{EmbedData, PPMissingEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use rosu::{backend::requests::BestRequest, models::GameMode};
use std::sync::Arc;
use twilight::model::channel::Message;

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
    } else if pp.is_infinite() {
        let content = "Number too large";
        return msg.error(&ctx, content).await;
    }

    // Retrieve the user and their top scores
    let scores_fut = BestRequest::with_username(&name)
        .mode(mode)
        .limit(100)
        .queue(ctx.osu());
    let join_result = tokio::try_join!(ctx.osu_user(&name, mode), scores_fut);
    let (user, scores) = match join_result {
        Ok((Some(user), scores)) => (user, scores),
        Ok((None, _)) => {
            let content = format!("User `{}` was not found", name);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };

    // Accumulate all necessary data
    let data = PPMissingEmbed::new(user, scores, pp);

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
