use crate::{
    arguments::{Args, NameIntArgs},
    custom_client::RankLeaderboard,
    embeds::{EmbedData, RankRankedScoreEmbed},
    util::{
        constants::{OSU_API_ISSUE, OSU_WEB_ISSUE},
        MessageExt,
    },
    BotResult, Context,
};

use futures::future::TryFutureExt;
use rosu::model::GameMode;
use std::sync::Arc;
use twilight_model::channel::Message;

async fn rank_score_main(
    mode: GameMode,
    ranking: Ranking,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = NameIntArgs::new(&ctx, args);
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };
    let rank = match args.number {
        Some(n) => n as usize,
        None => {
            let content = "You must specify a target rank.";
            return msg.error(&ctx, content).await;
        }
    };
    if rank == 0 {
        let content = "Rank number must be between 1 and 10,000";
        return msg.error(&ctx, content).await;
    } else if rank > 10_000 {
        let content = "Unfortunately I can only provide data for ranks up to 10,000 :(";
        return msg.error(&ctx, content).await;
    }

    let user_id_fut = match ranking {
        Ranking::Ranked => ctx
            .clients
            .custom
            .get_userid_of_rank(rank, RankLeaderboard::score(mode)),
    };

    // Retrieve the user and the id of the rank-holding user
    let (rank_holder_id_result, user_result) = tokio::join!(
        user_id_fut,
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

    // Accumulate all necessary data
    let data = RankRankedScoreEmbed::new(user, rank, rank_holder);

    // Creating the embed
    let embed = data.build_owned().build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}

#[command]
#[short_desc("How much ranked score is a player missing to reach the given rank?")]
#[long_desc(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[aliases("rrs")]
pub async fn rankrankedscore(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rank_score_main(GameMode::STD, Ranking::Ranked, ctx, msg, args).await
}

#[command]
#[short_desc("How much ranked score is a player missing to reach the given rank?")]
#[long_desc(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[aliases("rrsm")]
pub async fn rankrankedscoremania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rank_score_main(GameMode::MNA, Ranking::Ranked, ctx, msg, args).await
}

#[command]
#[short_desc("How much ranked score is a player missing to reach the given rank?")]
#[long_desc(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[aliases("rrst")]
pub async fn rankrankedscoretaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rank_score_main(GameMode::TKO, Ranking::Ranked, ctx, msg, args).await
}

#[command]
#[short_desc("How much ranked score is a player missing to reach the given rank?")]
#[long_desc(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[aliases("rrsc")]
pub async fn rankrankedscorectb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rank_score_main(GameMode::CTB, Ranking::Ranked, ctx, msg, args).await
}

enum Ranking {
    // Total,
    Ranked,
}
