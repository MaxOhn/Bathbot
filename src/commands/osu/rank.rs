use super::require_link;
use crate::{
    arguments::{Args, RankArgs},
    embeds::{EmbedData, RankEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context, Error,
};

use futures::future::TryFutureExt;
use rosu::{
    backend::requests::UserRequest,
    models::{GameMode, Score, User},
};
use std::sync::Arc;
use twilight::model::channel::Message;

async fn rank_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = match RankArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => return msg.respond(&ctx, err_msg).await,
    };
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return require_link(&ctx, msg).await,
    };
    let country = args.country;
    let rank = args.rank;

    // Retrieve the user and the id of the rank-holding user
    let rank_holder_id_fut = ctx
        .clients
        .custom
        .get_userid_of_rank(rank, mode, country.as_deref());
    let (rank_holder_id_result, user_result) = tokio::join!(
        rank_holder_id_fut,
        ctx.osu_user(&name, mode).map_err(|e| e.into())
    );
    let rank_holder_id = match rank_holder_id_result {
        Ok(id) => id,
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why);
        }
    };
    let user = match user_result {
        Ok(Some(user)) => user,
        Ok(None) => {
            let content = format!("User `{}` was not found", name);
            return msg.respond(&ctx, content).await;
        }
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why);
        }
    };

    // Retrieve rank-holding user
    let req = UserRequest::with_user_id(rank_holder_id).mode(mode);
    let rank_holder = match req.queue_single(&ctx.clients.osu).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let content = format!("User id `{}` was not found", rank_holder_id);
            return msg.respond(&ctx, content).await;
        }
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why.into());
        }
    };

    // Retrieve the user's top scores if required
    let scores = if user.pp_raw > rank_holder.pp_raw {
        None
    } else {
        match user.get_top_scores(&ctx.clients.osu, 100, mode).await {
            Ok(scores) => Some(scores),
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        }
    };

    // Accumulate all necessary data
    let data = RankEmbed::new(user, scores, rank, country, rank_holder);

    // Creating the embed
    let embed = data.build().build();
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[usage("[username] [[country]number]")]
#[example("badewanne3 be50")]
#[example("badewanne3 123")]
#[aliases("reach")]
pub async fn rank(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rank_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[example("badewanne3 be50")]
#[example("badewanne3 123")]
#[aliases("rankm", "reachmania", "reachm")]
pub async fn rankmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rank_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[example("badewanne3 be50")]
#[example("badewanne3 123")]
#[aliases("rankt", "reachtaiko", "reacht")]
pub async fn ranktaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rank_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[example("badewanne3 be50")]
#[example("badewanne3 123")]
#[aliases("rankc", "reachctb", "reachc")]
pub async fn rankctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rank_main(GameMode::CTB, ctx, msg, args).await
}
