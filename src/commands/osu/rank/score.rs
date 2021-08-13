use crate::{
    embeds::{EmbedData, RankRankedScoreEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    Args, BotResult, CommandData, Context, Name,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;

pub(super) async fn _rankscore(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: ScoreArgs,
) -> BotResult<()> {
    let ScoreArgs { name, mode, rank } = args;

    let name = match name {
        Some(name) => name,
        None => match ctx.get_link(data.author()?.id.0) {
            Some(name) => name,
            None => return super::require_link(&ctx, &data).await,
        },
    };

    if rank == 0 {
        let content = "Rank number must be between 1 and 10,000";

        return data.error(&ctx, content).await;
    } else if rank > 10_000 {
        let content = "Unfortunately I can only provide data for ranks up to 10,000 :(";

        return data.error(&ctx, content).await;
    }

    // Retrieve the user and the user thats holding the given rank
    let page = (rank / 50) + (rank % 50 != 0) as usize;
    let rank_holder_fut = ctx.osu().score_rankings(mode).page(page as u32);
    let user_fut = super::request_user(&ctx, &name, Some(mode));

    let (user, rank_holder) = match tokio::try_join!(user_fut, rank_holder_fut) {
        Ok((user, mut rankings)) => {
            let idx = (rank + 49) % 50;
            let rank_holder = rankings.ranking.swap_remove(idx);

            (user, rank_holder)
        }
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Accumulate all necessary data
    let embed_data = RankRankedScoreEmbed::new(user, rank, rank_holder);

    // Creating the embed
    let embed = embed_data.into_builder().build();
    data.create_message(&ctx, embed.into()).await?;

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
pub async fn rankrankedscore(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ScoreArgs::args(&ctx, &mut args, GameMode::STD) {
                Ok(rank_args) => {
                    _rankscore(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_rank(ctx, command).await,
    }
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
pub async fn rankrankedscoremania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ScoreArgs::args(&ctx, &mut args, GameMode::MNA) {
                Ok(rank_args) => {
                    _rankscore(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_rank(ctx, command).await,
    }
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
pub async fn rankrankedscoretaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ScoreArgs::args(&ctx, &mut args, GameMode::TKO) {
                Ok(rank_args) => {
                    _rankscore(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_rank(ctx, command).await,
    }
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
pub async fn rankrankedscorectb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ScoreArgs::args(&ctx, &mut args, GameMode::CTB) {
                Ok(rank_args) => {
                    _rankscore(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_rank(ctx, command).await,
    }
}

pub(super) struct ScoreArgs {
    pub name: Option<Name>,
    pub mode: GameMode,
    pub rank: usize,
}

impl ScoreArgs {
    fn args(ctx: &Context, args: &mut Args<'_>, mode: GameMode) -> Result<Self, &'static str> {
        let mut name = None;
        let mut rank = None;

        for arg in args.take(2) {
            match arg.parse() {
                Ok(num) => rank = Some(num),
                Err(_) => name = Some(Args::try_link_name(ctx, arg)?),
            }
        }

        let rank = rank.ok_or("You must specify a target rank.")?;

        Ok(Self { name, mode, rank })
    }
}
