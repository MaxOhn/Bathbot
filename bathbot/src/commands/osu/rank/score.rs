use std::{borrow::Cow, cmp, fmt::Write, iter, sync::Arc};

use bathbot_macros::command;
use bathbot_model::{rosu_v2::user::User, RespektiveUser};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE, OSU_BASE},
    matcher,
    numbers::WithComma,
    osu::flag_url,
    AuthorBuilder, CowUtils, EmbedBuilder, MessageBuilder,
};
use eyre::{Report, Result};
use rosu_v2::prelude::{OsuError, UserId, Username};

use super::{RankScore, RankValue};
use crate::{
    commands::{osu::user_not_found, GameModeOption},
    core::commands::{prefix::Args, CommandOrigin},
    manager::redis::{osu::UserArgs, RedisData},
    util::ChannelExt,
    Context,
};

#[command]
#[desc("How much ranked score is a player missing to reach the given rank?")]
#[help(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000.\n\
    If no number is given, one of the arguments will be considered as username whose rank should be reached."
)]
#[usage("[username] [number/username]")]
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
    The number for the rank must be between 1 and 10,000.\n\
    If no number is given, one of the arguments will be considered as username whose rank should be reached."
)]
#[usage("[username] [number/username]")]
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
    The number for the rank must be between 1 and 10,000.\n\
    If no number is given, one of the arguments will be considered as username whose rank should be reached."
)]
#[usage("[username] [number/username]")]
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
    The number for the rank must be between 1 and 10,000.\n\
    If no number is given, one of the arguments will be considered as username whose rank should be reached."
)]
#[usage("[username] [number/username]")]
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
            } else if name.is_some() {
                rank = Some(arg);
            } else {
                name = Some(arg.into());
            }
        }

        let rank = rank.map(Cow::Borrowed).or_else(|| name.take()).ok_or(
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
    let (user_id, mode) = user_id_mode!(ctx, orig, args);
    let rank_value = RankValue::parse(args.rank.as_ref());

    if matches!(rank_value, RankValue::Raw(0)) {
        let content = "Rank number must be between 1 and 10,000";

        return orig.error(&ctx, content).await;
    } else if matches!(rank_value, RankValue::Delta(0)) {
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

    let mut rank_holder = None;
    let mut reach_name = false;

    let rank = match rank_value {
        RankValue::Delta(delta) => {
            let user_fut = ctx
                .client()
                .get_respektive_users(iter::once(user.user_id()), mode);

            let curr_rank = match user_fut
                .await
                .map(|mut users| users.next().flatten().and_then(|user| user.rank))
            {
                Ok(Some(rank)) => rank.get(),
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
        RankValue::Name(name) => {
            let user_id = match UserArgs::username(&ctx, name).await {
                UserArgs::Args(args) => args.user_id,
                UserArgs::User { user, .. } => {
                    rank_holder = Some(RankHolder {
                        ranked_score: user.statistics.map_or(0, |stats| stats.ranked_score),
                        username: user.username,
                    });

                    user.user_id
                }
                UserArgs::Err(OsuError::NotFound) => {
                    let content = user_not_found(&ctx, UserId::from(name)).await;

                    return orig.error(&ctx, content).await;
                }
                UserArgs::Err(err) => {
                    let _ = orig.error(&ctx, OSU_API_ISSUE).await;

                    return Err(Report::new(err).wrap_err("Failed to get target user"));
                }
            };

            let user_fut = ctx.client().get_respektive_users(iter::once(user_id), mode);

            let rank_opt = match user_fut.await {
                Ok(mut users) => users.next().flatten().and_then(|user| user.rank),
                Err(err) => {
                    let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err.wrap_err("Failed to get respektive user"));
                }
            };

            let Some(rank) = rank_opt else {
                let content = format!(
                    "Failed to get score rank data for user `{name}`.\n\
                    In order for a target name to work, \
                    the user must be at least top 10k in the score ranking.",
                );

                return orig.error(&ctx, content).await;
            };

            reach_name = true;

            rank.get()
        }
    };

    if rank > 10_000 {
        let content = "Unfortunately I can only provide data for ranks up to 10,000 :(";

        return orig.error(&ctx, content).await;
    }

    // Retrieve the user and the user thats holding the given rank
    let rank_holder = if let Some(rank_holder) = rank_holder {
        rank_holder
    } else {
        let page = (rank as usize / 50) + (rank % 50 != 0) as usize;
        let rank_holder_fut = ctx.osu().score_rankings(mode).page(page as u32);

        match rank_holder_fut.await {
            Ok(mut rankings) => {
                let idx = (rank as usize + 49) % 50;
                let user = rankings.ranking.swap_remove(idx);

                RankHolder {
                    ranked_score: user.statistics.map_or(0, |stats| stats.ranked_score),
                    username: user.username,
                }
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

    let username = user.username().cow_escape_markdown();

    let title = if reach_name {
        let holder_name = rank_holder.username.as_str();

        format!(
            "How much ranked score is {username} missing to reach \
            {holder_name}'{genitiv} rank #{rank}?",
            genitiv = if holder_name.ends_with('s') { "" } else { "s" },
        )
    } else {
        format!("How much ranked score is {username} missing to reach rank #{rank}?")
    };

    let user_score = user.stats().ranked_score();
    let rank_holder_score = rank_holder.ranked_score;

    let mut description = if reach_name {
        format!(
            "{name} is rank {rank} with **{score}** ranked score",
            name = rank_holder.username.cow_escape_markdown(),
            score = WithComma::new(rank_holder_score),
        )
    } else {
        format!(
            "Rank #{rank} is currently held by {name} with **{score}** ranked score",
            name = rank_holder.username.cow_escape_markdown(),
            score = WithComma::new(rank_holder_score),
        )
    };

    if user_score > rank_holder_score {
        let _ = write!(
            description,
            ", so {username} is already above that with **{score} ranked score**.",
            score = WithComma::new(user_score)
        );
    } else {
        let _ = write!(
            description,
            ", so {username} is missing **{missing}** score.",
            missing = WithComma::new(rank_holder_score - user_score),
        );
    }

    let embed = EmbedBuilder::new()
        .author(author(&user, respektive_user.as_ref()))
        .description(description)
        .thumbnail(user.avatar_url().to_owned())
        .title(title);

    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(&ctx, builder).await?;

    Ok(())
}

struct RankHolder {
    ranked_score: u64,
    username: Username,
}

fn author(user: &RedisData<User>, respektive_user: Option<&RespektiveUser>) -> AuthorBuilder {
    let rank = respektive_user.and_then(|user| user.rank);

    let mut text = format!(
        "{username}: {score}",
        username = user.username(),
        score = WithComma::new(user.stats().ranked_score()),
    );

    if let Some(rank) = rank {
        let _ = write!(text, " (#{})", WithComma::new(rank.get()));
    }

    let (country_code, user_id, mode) = match user {
        RedisData::Original(user) => {
            let country_code = user.country_code.as_str();
            let user_id = user.user_id;
            let mode = user.mode;

            (country_code, user_id, mode)
        }
        RedisData::Archive(user) => {
            let country_code = user.country_code.as_str();
            let user_id = user.user_id;
            let mode = user.mode;

            (country_code, user_id, mode)
        }
    };

    let url = format!("{OSU_BASE}users/{user_id}/{mode}");
    let icon = flag_url(country_code);

    AuthorBuilder::new(text).url(url).icon_url(icon)
}
