use super::request_user;
use crate::{
    arguments::{Args, RankArgs},
    custom_client::{ManiaVariant, RankLeaderboard, RankParam},
    embeds::{EmbedData, RankEmbed},
    tracking::process_tracking,
    util::{
        constants::{OSU_API_ISSUE, OSU_DAILY_ISSUE, OSU_WEB_ISSUE},
        MessageExt,
    },
    BotResult, Context,
};

use rosu_v2::prelude::{GameMode, OsuError, User};
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
        // Retrieve the user and the id of the rank-holding user
        let mut ranking = RankLeaderboard::pp(mode, args.country.as_deref());

        match (mode, args.variant) {
            (GameMode::MNA, Some(ManiaVariant::K4)) => ranking = ranking.variant_4k(),
            (GameMode::MNA, Some(ManiaVariant::K7)) => ranking = ranking.variant_7k(),
            _ => {}
        }

        let rank_holder_id_fut = ctx.clients.custom.get_userid_of_rank(args.rank, ranking);
        let user_fut = request_user(&ctx, &name, Some(mode));

        let (rank_holder_id_result, user_result) = tokio::join!(rank_holder_id_fut, user_fut,);

        let rank_holder_id = match rank_holder_id_result {
            Ok(id) => id,
            Err(why) => {
                let _ = msg.error(&ctx, OSU_WEB_ISSUE).await;

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

        // Retrieve rank-holding user
        let rank_holder = match ctx.osu().user(rank_holder_id).mode(mode).await {
            Ok(user) => user,
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
        let user = data.user();

        let scores_fut_1 = ctx
            .osu()
            .user_scores(user.user_id)
            .limit(50)
            .best()
            .mode(mode);

        let scores_fut_2 = ctx
            .osu()
            .user_scores(user.user_id)
            .offset(50)
            .limit(50)
            .best()
            .mode(mode);

        match tokio::try_join!(scores_fut_1, scores_fut_2) {
            Ok((mut scores, mut scores_2)) => (!scores.is_empty()).then(|| {
                scores.append(&mut scores_2);

                scores
            }),
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        }
    } else {
        None
    };

    if let Some(scores) = scores.as_deref_mut() {
        // Process user and their top scores for tracking
        process_tracking(&ctx, mode, scores, Some(data.user())).await;
    }

    // Creating the embed
    let embed = RankEmbed::new(data, scores).build_owned().build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await?;

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
    For ranks up to 10,000 you can also specify `+4k` or `+7k` for those specific leaderboard.\n\
    For ranks over 10,000 the data is provided by [osudaily](https://osudaily.net/)."
)]
#[usage("[username] [+4k/+7k] [[country]number]")]
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
        rank_holder: User,
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

    pub fn user(&self) -> &User {
        match self {
            Self::Sub10k { user, .. } => user,
            Self::Over10k { user, .. } => user,
        }
    }
}
