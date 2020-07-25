use super::require_link;
use crate::{
    arguments::{Args, NameFloatArgs},
    embeds::{EmbedData, PPMissingEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use rosu::{
    backend::requests::UserRequest,
    models::{GameMode, Score, User},
};
use std::sync::Arc;
use twilight::model::channel::Message;

async fn pp_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = match NameFloatArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => {
            return msg.respond(&ctx, err_msg).await;
        }
    };
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return require_link(&ctx, msg).await,
    };
    let pp = args.float;
    if pp < 0.0 {
        let content = "The pp number must be non-negative";
        return msg.respond(&ctx, content).await;
    }

    // Retrieve the user and their top scores
    let user = match ctx.osu_user(&name, mode).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let content = format!("User `{}` was not found", name);
            return msg.respond(&ctx, content).await;
        }
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why.into());
        }
    };
    let scores = match user.get_top_scores(&ctx.clients.osu, 100, mode).await {
        Ok(scores) => scores,
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why.into());
        }
    };

    // Accumulate all necessary data
    let data = PPMissingEmbed::new(user, scores, pp);

    // Creating the embed
    let embed = data.build().build();
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
