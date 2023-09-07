use std::{borrow::Cow, cmp, iter, sync::Arc};

use bathbot_macros::command;
use bathbot_util::{constants::OSU_API_ISSUE, matcher, MessageBuilder};
use eyre::{Report, Result};
use rosu_v2::prelude::OsuError;

use super::{RankScore, RankValue};
use crate::{
    commands::{osu::user_not_found, GameModeOption},
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, RankRankedScoreEmbed},
    manager::redis::osu::UserArgs,
    util::ChannelExt,
    Context,
};

#[command]
#[desc("How much ranked score is a player missing to reach the given rank?")]
#[help(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[alias("rrs")]
#[group(Osu)]
async fn prefix_rankrankedscore(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RankScore::args(None, args) {
        Ok(args) => score(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How much ranked score is a player missing to reach the given rank?")]
#[help(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[alias("rrsm")]
#[group(Mania)]
async fn prefix_rankrankedscoremania(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> Result<()> {
    match RankScore::args(Some(GameModeOption::Mania), args) {
        Ok(args) => score(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How much ranked score is a player missing to reach the given rank?")]
#[help(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[alias("rrst")]
#[group(Taiko)]
async fn prefix_rankrankedscoretaiko(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> Result<()> {
    match RankScore::args(Some(GameModeOption::Taiko), args) {
        Ok(args) => score(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How much ranked score is a player missing to reach the given rank?")]
#[help(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[aliases("rrsc", "rankrankedscorecatch")]
#[group(Catch)]
async fn prefix_rankrankedscorectb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RankScore::args(Some(GameModeOption::Catch), args) {
        Ok(args) => score(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

impl<'m> RankScore<'m> {
    fn args(mode: Option<GameModeOption>, args: Args<'m>) -> Result<Self, &'static str> {
        let mut name = None;
        let mut discord = None;
        let mut rank = None;

        for arg in args.take(2) {
            if arg.parse::<u32>().is_ok() {
                rank = Some(arg);
            } else if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        let rank = rank.map(Cow::Borrowed).ok_or(
            "Failed to parse `rank`. Provide it either as positive number \
            or as country acronym followed by a positive number e.g. `be10` \
            as one of the first two arguments.",
        )?;

        Ok(Self {
            rank,
            mode,
            name,
            discord,
        })
    }
}

pub(super) async fn score(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: RankScore<'_>,
) -> Result<()> {
    let Some(rank) = RankValue::parse(args.rank.as_ref()) else {
        let content = "Failed to parse rank. Be sure to specify a positive integer.";

        return orig.error(&ctx, content).await;
    };

    let (user_id, mode) = user_id_mode!(ctx, orig, args);

    if matches!(rank, RankValue::Raw(0)) {
        let content = "Rank number must be between 1 and 10,000";

        return orig.error(&ctx, content).await;
    } else if matches!(rank, RankValue::Delta(0)) {
        return orig
            .error(&ctx, "Delta must be greater than zero :clown:")
            .await;
    }

    let user_args = UserArgs::rosu_id(&ctx, &user_id).await.mode(mode);

    let user = match ctx.redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get user"));
        }
    };

    let rank = match rank {
        RankValue::Delta(delta) => {
            let user_fut = ctx
                .client()
                .get_respektive_users(iter::once(user.user_id()), mode);

            let curr_rank = match user_fut.await.map(|mut users| users.next().flatten()) {
                Ok(Some(user)) => user.rank,
                Ok(None) => {
                    let content = format!(
                        "Failed to get score rank data for user `{}`.\n\
                        In order for delta input to work, \
                        the user must be at least top 10k in the score ranking.",
                        user.username()
                    );

                    return orig.error(&ctx, content).await;
                }
                Err(err) => {
                    let _ = orig.error(&ctx, "Some issue with respektive's api").await;

                    return Err(err.wrap_err("Failed to get respektive user"));
                }
            };

            cmp::max(1, curr_rank.saturating_sub(delta))
        }
        RankValue::Raw(rank) => rank,
    };

    if rank > 10_000 {
        let content = "Unfortunately I can only provide data for ranks up to 10,000 :(";

        return orig.error(&ctx, content).await;
    }

    // Retrieve the user and the user thats holding the given rank
    let page = (rank as usize / 50) + (rank % 50 != 0) as usize;
    let rank_holder_fut = ctx.osu().score_rankings(mode).page(page as u32);

    let rank_holder = match rank_holder_fut.await {
        Ok(mut rankings) => {
            let idx = (rank as usize + 49) % 50;

            rankings.ranking.swap_remove(idx)
        }
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let rank_fut = ctx
        .client()
        .get_respektive_users(iter::once(user.user_id()), mode);

    let respektive_user = match rank_fut.await {
        Ok(mut iter) => iter.next().flatten(),
        Err(err) => {
            warn!(?err, "Failed to get respektive user");

            None
        }
    };

    // Accumulate all necessary data
    let embed_data = RankRankedScoreEmbed::new(&user, rank, rank_holder, respektive_user);

    // Creating the embed
    let embed = embed_data.build();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(&ctx, builder).await?;

    Ok(())
}
