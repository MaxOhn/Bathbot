use super::request_user;
use crate::{
    arguments::{Args, NameIntArgs},
    embeds::{EmbedData, RankRankedScoreEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;
use twilight_model::channel::Message;

async fn rank_score_main(
    mode: GameMode,
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

    // Retrieve the user and the user thats holding the given rank
    let page = (rank / 50) + (rank % 50 != 0) as usize;
    let rank_holder_fut = ctx.osu().score_rankings(mode).page(page as u32);
    let user_fut = request_user(&ctx, &name, Some(mode));

    let (user, rank_holder) = match tokio::try_join!(user_fut, rank_holder_fut) {
        Ok((user, mut rankings)) => {
            let idx = (rank + 49) % 50;
            let rank_holder = rankings.ranking.swap_remove(idx);

            (user, rank_holder)
        }
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

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
    let embed = data.into_builder().build();
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
    rank_score_main(GameMode::STD, ctx, msg, args).await
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
    rank_score_main(GameMode::MNA, ctx, msg, args).await
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
    rank_score_main(GameMode::TKO, ctx, msg, args).await
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
    rank_score_main(GameMode::CTB, ctx, msg, args).await
}
