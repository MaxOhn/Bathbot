use std::sync::Arc;

use bathbot_macros::command;
use eyre::{Report, Result};
use rkyv::{Deserialize, Infallible};
use rosu_v2::prelude::{OsuError, UserCompact};

use crate::{
    commands::{osu::user_not_found, GameModeOption},
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, RankEmbed},
    manager::redis::{
        osu::{User, UserArgs},
        RedisData,
    },
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher, ChannelExt, CountryCode,
    },
    Context,
};

use super::RankPp;

pub(super) async fn pp(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: RankPp<'_>) -> Result<()> {
    let (user_id, mode) = user_id_mode!(ctx, orig, args);

    let RankPp {
        country,
        rank,
        each,
        ..
    } = args;

    let country = match country {
        Some(country) => match CountryCode::from_name(&country) {
            Some(code) => Some(code),
            None => {
                if country.len() == 2 {
                    Some(CountryCode::from(country))
                } else {
                    let content = format!(
                        "Looks like `{country}` is neither a country name nor a country code"
                    );

                    return orig.error(&ctx, content).await;
                }
            }
        },
        None => None,
    };

    if rank == 0 {
        return orig.error(&ctx, "Rank can't be zero :clown:").await;
    } else if rank > 10_000 && country.is_some() {
        let content = "Unfortunately I can only provide data for country ranks up to 10,000 :(";

        return orig.error(&ctx, content).await;
    }

    let rank_data = if rank <= 10_000 {
        // Retrieve the user and the user thats holding the given rank
        let page = (rank / 50) + (rank % 50 != 0) as u32;

        let rank_holder_fut =
            ctx.redis()
                .pp_ranking(mode, page, country.as_ref().map(|c| c.as_str()));

        let user_args = UserArgs::rosu_id(&ctx, &user_id).await.mode(mode);
        let user_fut = ctx.redis().osu_user(user_args);

        let (user, rank_holder) = match tokio::try_join!(user_fut, rank_holder_fut) {
            Ok((user, rankings)) => {
                let idx = ((args.rank + 49) % 50) as usize;

                let rank_holder = match rankings {
                    RedisData::Original(mut rankings) => rankings.ranking.swap_remove(idx),
                    RedisData::Archived(rankings) => {
                        rankings.ranking[idx].deserialize(&mut Infallible).unwrap()
                    }
                };

                (user, rank_holder)
            }
            Err(OsuError::NotFound) => {
                let content = user_not_found(&ctx, user_id).await;

                return orig.error(&ctx, content).await;
            }
            Err(err) => {
                let _ = orig.error(&ctx, OSU_API_ISSUE).await;
                let err = Report::new(err).wrap_err("failed to get user");

                return Err(err);
            }
        };

        RankData::Sub10k {
            user,
            rank,
            country,
            rank_holder: Box::new(rank_holder),
        }
    } else {
        let pp_fut = ctx.approx().pp(rank, mode);

        let user_args = UserArgs::rosu_id(&ctx, &user_id).await.mode(mode);
        let user_fut = ctx.redis().osu_user(user_args);

        let (pp_res, user_res) = tokio::join!(pp_fut, user_fut);

        let required_pp = match pp_res {
            Ok(pp) => pp,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        };

        let user = match user_res {
            Ok(user) => user,
            Err(OsuError::NotFound) => {
                let content = user_not_found(&ctx, user_id).await;

                return orig.error(&ctx, content).await;
            }
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;
                let err = Report::new(err).wrap_err("failed to get user");

                return Err(err);
            }
        };

        RankData::Over10k {
            user,
            rank: args.rank,
            required_pp,
        }
    };

    // Retrieve the user's top scores if required
    let scores = if rank_data.with_scores() {
        let user = rank_data.user();

        let scores_fut = ctx
            .osu()
            .user_scores(user.user_id())
            .limit(100)
            .best()
            .mode(mode);

        match scores_fut.await {
            Ok(scores) => (!scores.is_empty()).then_some(scores),
            Err(err) => {
                let _ = orig.error(&ctx, OSU_API_ISSUE).await;
                let report = Report::new(err).wrap_err("failed to get scores");

                return Err(report);
            }
        }
    } else {
        None
    };

    // Creating the embed
    let embed = RankEmbed::new(rank_data, scores, each).build();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(&ctx, &builder).await?;

    Ok(())
}

#[command]
#[desc("How many pp is a player missing to reach the given rank?")]
#[help(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the value is an approximation based on cached user data."
)]
#[usage("[username] [[country]number]")]
#[examples("badewanne3 be50", "badewanne3 123")]
#[alias("reach")]
#[group(Osu)]
async fn prefix_rank(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RankPp::args(None, args) {
        Ok(args) => pp(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many pp is a player missing to reach the given rank?")]
#[help(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the value is an approximation based on cached user data."
)]
#[usage("[username] [[country]number]")]
#[examples("badewanne3 be50", "badewanne3 123")]
#[alias("rankm", "reachmania", "reachm")]
#[group(Mania)]
async fn prefix_rankmania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RankPp::args(Some(GameModeOption::Mania), args) {
        Ok(args) => pp(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many pp is a player missing to reach the given rank?")]
#[help(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the value is an approximation based on cached user data."
)]
#[usage("[username] [[country]number]")]
#[examples("badewanne3 be50", "badewanne3 123")]
#[alias("rankt", "reachtaiko", "reacht")]
#[group(Taiko)]
async fn prefix_ranktaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RankPp::args(Some(GameModeOption::Taiko), args) {
        Ok(args) => pp(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many pp is a player missing to reach the given rank?")]
#[help(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the value is an approximation based on cached user data."
)]
#[usage("[username] [[country]number]")]
#[examples("badewanne3 be50", "badewanne3 123")]
#[alias("rankc", "reachctb", "reachc", "rankcatch", "reachcatch")]
#[group(Catch)]
async fn prefix_rankctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RankPp::args(Some(GameModeOption::Catch), args) {
        Ok(args) => pp(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

pub enum RankData {
    Sub10k {
        user: RedisData<User>,
        rank: u32,
        country: Option<CountryCode>,
        rank_holder: Box<UserCompact>,
    },
    Over10k {
        user: RedisData<User>,
        rank: u32,
        required_pp: f32,
    },
}

impl RankData {
    fn with_scores(&self) -> bool {
        match self {
            Self::Sub10k {
                user, rank_holder, ..
            } => user.peek_stats(|stats| stats.pp < rank_holder.statistics.as_ref().unwrap().pp),
            Self::Over10k {
                user, required_pp, ..
            } => user.peek_stats(|stats| stats.pp < *required_pp),
        }
    }

    pub fn user(&self) -> &RedisData<User> {
        match self {
            Self::Sub10k { user, .. } => user,
            Self::Over10k { user, .. } => user,
        }
    }
}

impl<'m> RankPp<'m> {
    fn args(mode: Option<GameModeOption>, args: Args<'m>) -> Result<Self, &'static str> {
        let mut name = None;
        let mut discord = None;
        let mut country = None;
        let mut rank = None;

        for arg in args.take(2) {
            if let Ok(num) = arg.parse() {
                rank = Some(num);

                continue;
            } else if arg.len() >= 3 {
                let (prefix, suffix) = arg.split_at(2);
                let valid_country = prefix.chars().all(|c| c.is_ascii_alphabetic());

                if let (true, Ok(num)) = (valid_country, suffix.parse()) {
                    country = Some(prefix.to_ascii_uppercase().into());
                    rank = Some(num);

                    continue;
                }
            }

            if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        let rank = rank.ok_or(
            "Failed to parse `rank`. Provide it either as positive number \
            or as country acronym followed by a positive number e.g. `be10` \
            as one of the first two arguments.",
        )?;

        Ok(Self {
            rank,
            mode,
            name,
            each: None,
            country,
            discord,
        })
    }
}
