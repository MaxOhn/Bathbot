use crate::{
    arguments::{Args, RankArgs},
    embeds::{EmbedData, RankEmbed},
    tracking::process_tracking,
    util::{
        constants::{OSU_API_ISSUE, OSU_WEB_ISSUE},
        MessageExt,
    },
    BotResult, Context,
};

use futures::future::TryFutureExt;
use rosu::model::GameMode;
use std::{collections::HashMap, sync::Arc};
use twilight_model::channel::Message;

async fn rank_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = match RankArgs::new(&ctx, args) {
        Ok(args) => args,
        Err(err_msg) => return msg.error(&ctx, err_msg).await,
    };
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };
    let country = args.country;
    let rank = args.rank;
    if rank == 0 {
        let content = "Rank number must be between 1 and 10,000";
        return msg.error(&ctx, content).await;
    } else if rank > 10_000 {
        let content = "Unfortunately I can only provide data for ranks up to 10,000 :(";
        return msg.error(&ctx, content).await;
    }

    // Retrieve the user and the id of the rank-holding user
    let rank_holder_id_fut = ctx
        .clients
        .custom
        .get_userid_of_rank(rank, mode, country.as_deref());
    let (rank_holder_id_result, user_result) = tokio::join!(
        rank_holder_id_fut,
        ctx.osu()
            .user(name.as_str())
            .mode(mode)
            .map_err(|e| e.into())
    );
    let rank_holder_id = match rank_holder_id_result {
        Ok(id) => id,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_WEB_ISSUE).await;
            return Err(why.into());
        }
    };
    let user = match user_result {
        Ok(Some(user)) => user,
        Ok(None) => {
            let content = format!("User `{}` was not found", name);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why);
        }
    };

    // Retrieve rank-holding user
    let rank_holder = match ctx.osu().user(rank_holder_id).mode(mode).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let content = format!("User id `{}` was not found", rank_holder_id);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };

    // Retrieve the user's top scores if required
    let scores = if user.pp_raw > rank_holder.pp_raw {
        None
    } else {
        match user.get_top_scores(ctx.osu()).limit(100).mode(mode).await {
            Ok(scores) if scores.is_empty() => None,
            Ok(scores) => Some(scores),
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;
                return Err(why.into());
            }
        }
    };

    if let Some(ref scores) = scores {
        // Process user and their top scores for tracking
        let mut maps = HashMap::new();
        process_tracking(&ctx, mode, scores, Some(&user), &mut maps).await;
    }

    // Accumulate all necessary data
    let data = RankEmbed::new(user, scores, rank, country, rank_holder);

    // Creating the embed
    let embed = data.build().build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[long_desc(
    "How many pp is a player missing to reach the given rank?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [[country]number]")]
#[example("badewanne3 be50", "badewanne3 123")]
#[aliases("reach")]
pub async fn rank(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rank_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[long_desc(
    "How many pp is a player missing to reach the given rank?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[example("badewanne3 be50", "badewanne3 123")]
#[aliases("rankm", "reachmania", "reachm")]
pub async fn rankmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rank_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[long_desc(
    "How many pp is a player missing to reach the given rank?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[example("badewanne3 be50", "badewanne3 123")]
#[aliases("rankt", "reachtaiko", "reacht")]
pub async fn ranktaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rank_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[long_desc(
    "How many pp is a player missing to reach the given rank?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[example("badewanne3 be50", "badewanne3 123")]
#[aliases("rankc", "reachctb", "reachc")]
pub async fn rankctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rank_main(GameMode::CTB, ctx, msg, args).await
}
