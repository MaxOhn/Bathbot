use super::request_user;
use crate::{
    arguments::{Args, RankArgs},
    custom_client::RankParam,
    embeds::{EmbedData, RankEmbed},
    tracking::process_tracking,
    util::{
        constants::{OSU_API_ISSUE, OSU_DAILY_ISSUE},
        MessageExt,
    },
    BotResult, Context,
};

use rosu_v2::prelude::{GameMode, OsuError, User, UserCompact};
use std::sync::Arc;
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

    if args.rank == 0 {
        let content = "Rank can't be zero :clown:";

        return msg.error(&ctx, content).await;
    } else if args.rank > 10_000 && args.country.is_some() {
        let content = "Unfortunately I can only provide data for country ranks up to 10,000 :(";

        return msg.error(&ctx, content).await;
    }

    let data = if args.rank <= 10_000 {
        // Retrieve the user and the user thats holding the given rank
        let page = (args.rank / 50) + (args.rank % 50 != 0) as usize;

        let mut rank_holder_fut = ctx.osu().performance_rankings(mode).page(page as u32);

        if let Some(ref country) = args.country {
            rank_holder_fut = rank_holder_fut.country(country);
        }

        let user_fut = request_user(&ctx, &name, Some(mode));

        let (user, rank_holder) = match tokio::try_join!(user_fut, rank_holder_fut) {
            Ok((user, mut rankings)) => {
                let idx = (args.rank + 49) % 50;
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

        RankData::Sub10k {
            user,
            rank: args.rank,
            country: args.country,
            rank_holder,
        }
    } else {
        let pp_fut = ctx
            .clients
            .custom
            .get_rank_data(mode, RankParam::Rank(args.rank));

        let user_fut = request_user(&ctx, &name, Some(mode));
        let (pp_result, user_result) = tokio::join!(pp_fut, user_fut);

        let required_pp = match pp_result {
            Ok(rank_pp) => rank_pp.pp,
            Err(why) => {
                let _ = msg.error(&ctx, OSU_DAILY_ISSUE).await;

                return Err(why.into());
            }
        };

        let user = match user_result {
            Ok(user) => user,
            Err(OsuError::NotFound) => {
                let content = format!("User `{}` was not found", name);

                return msg.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        };

        RankData::Over10k {
            user,
            rank: args.rank,
            required_pp,
        }
    };

    // Retrieve the user's top scores if required
    let mut scores = if data.with_scores() {
        let user = data.user_borrow();

        let scores_fut = ctx
            .osu()
            .user_scores(user.user_id)
            .limit(100)
            .best()
            .mode(mode);

        match scores_fut.await {
            Ok(scores) => (!scores.is_empty()).then(|| scores),
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        }
    } else {
        None
    };

    if let Some(ref mut scores) = scores {
        // Process user and their top scores for tracking
        process_tracking(&ctx, mode, scores, Some(data.user_borrow())).await;
    }

    // Creating the embed
    let embed = &[RankEmbed::new(data, scores).into_builder().build()];
    msg.build_response(&ctx, |m| m.embeds(embed)).await?;

    Ok(())
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[long_desc(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the data is provided by [osudaily](https://osudaily.net/)."
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
    For ranks over 10,000 the data is provided by [osudaily](https://osudaily.net/)."
)]
#[usage("[username] [[country]number]")]
#[example("badewanne3 be50", "badewanne3 123")]
#[aliases("rankm", "reachmania", "reachm")]
pub async fn rankmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rank_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[long_desc(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the data is provided by [osudaily](https://osudaily.net/)."
)]
#[usage("[username] [[country]number]")]
#[example("badewanne3 be50", "badewanne3 123")]
#[aliases("rankt", "reachtaiko", "reacht")]
pub async fn ranktaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rank_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[long_desc(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the data is provided by [osudaily](https://osudaily.net/)."
)]
#[usage("[username] [[country]number]")]
#[example("badewanne3 be50", "badewanne3 123")]
#[aliases("rankc", "reachctb", "reachc")]
pub async fn rankctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rank_main(GameMode::CTB, ctx, msg, args).await
}

pub enum RankData {
    Sub10k {
        user: User,
        rank: usize,
        country: Option<String>,
        rank_holder: UserCompact,
    },
    Over10k {
        user: User,
        rank: usize,
        required_pp: f32,
    },
}

impl RankData {
    fn with_scores(&self) -> bool {
        match self {
            Self::Sub10k {
                user, rank_holder, ..
            } => user.statistics.as_ref().unwrap().pp < rank_holder.statistics.as_ref().unwrap().pp,
            Self::Over10k {
                user, required_pp, ..
            } => user.statistics.as_ref().unwrap().pp < *required_pp,
        }
    }

    pub fn user_borrow(&self) -> &User {
        match self {
            Self::Sub10k { user, .. } => user,
            Self::Over10k { user, .. } => user,
        }
    }

    pub fn user(self) -> User {
        match self {
            Self::Sub10k { user, .. } => user,
            Self::Over10k { user, .. } => user,
        }
    }
}
