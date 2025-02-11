use std::{borrow::Cow, cmp, fmt::Write, iter};

use bathbot_macros::command;
use bathbot_model::{command_fields::GameModeOption, RespektiveUser};
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
    commands::osu::user_not_found,
    core::commands::{prefix::Args, CommandOrigin},
    manager::redis::osu::{CachedUser, UserArgs, UserArgsError},
    util::{CachedUserExt, ChannelExt},
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
async fn prefix_rankrankedscore(msg: &Message, args: Args<'_>) -> Result<()> {
    match RankScore::args(None, args) {
        Ok(args) => score(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

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
async fn prefix_rankrankedscoremania(msg: &Message, args: Args<'_>) -> Result<()> {
    match RankScore::args(Some(GameModeOption::Mania), args) {
        Ok(args) => score(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

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
async fn prefix_rankrankedscoretaiko(msg: &Message, args: Args<'_>) -> Result<()> {
    match RankScore::args(Some(GameModeOption::Taiko), args) {
        Ok(args) => score(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

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
async fn prefix_rankrankedscorectb(msg: &Message, args: Args<'_>) -> Result<()> {
    match RankScore::args(Some(GameModeOption::Catch), args) {
        Ok(args) => score(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

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

pub(super) async fn score(orig: CommandOrigin<'_>, args: RankScore<'_>) -> Result<()> {
    let (user_id, mode) = user_id_mode!(orig, args);
    let rank_value = RankValue::parse(args.rank.as_ref());

    if matches!(rank_value, RankValue::Raw(0)) {
        let content = "Rank number must be between 1 and 10,000";

        return orig.error(content).await;
    } else if matches!(rank_value, RankValue::Delta(0)) {
        return orig.error("Delta must be greater than zero :clown:").await;
    }

    let user_args = UserArgs::rosu_id(&user_id, mode).await;

    let mut user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get user"));
        }
    };

    let mut rank_holder = None;
    let mut reach_name = false;

    let rank = match rank_value {
        RankValue::Delta(delta) => {
            let user_fut =
                Context::client().get_respektive_users(iter::once(user.user_id.to_native()), mode);

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
                        user.username.as_str()
                    );

                    return orig.error(content).await;
                }
                Err(err) => {
                    let _ = orig.error("Some issue with respektive's api").await;

                    return Err(err.wrap_err("Failed to get respektive user"));
                }
            };

            cmp::max(1, curr_rank.saturating_sub(delta))
        }
        RankValue::Raw(rank) => rank,
        RankValue::Name(name) => {
            let user_id = UserId::from(name);
            let user_args = UserArgs::rosu_id(&user_id, mode).await;

            let user_id = match Context::redis().osu_user(user_args).await {
                Ok(user) => {
                    rank_holder = Some(RankHolder {
                        ranked_score: user
                            .statistics
                            .as_ref()
                            .expect("missing stats")
                            .ranked_score
                            .to_native(),
                        username: user.username.as_str().into(),
                    });

                    user.user_id.to_native()
                }
                Err(UserArgsError::Osu(OsuError::NotFound)) => {
                    let content = user_not_found(UserId::from(name)).await;

                    return orig.error(content).await;
                }
                Err(err) => {
                    let _ = orig.error(GENERAL_ISSUE).await;

                    return Err(Report::new(err).wrap_err("Failed to get target user"));
                }
            };

            let user_fut = Context::client().get_respektive_users(iter::once(user_id), mode);

            let rank_opt = match user_fut.await {
                Ok(mut users) => users.next().flatten().and_then(|user| user.rank),
                Err(err) => {
                    let _ = orig.error(GENERAL_ISSUE).await;

                    return Err(err.wrap_err("Failed to get respektive user"));
                }
            };

            let Some(rank) = rank_opt else {
                let content = format!(
                    "Failed to get score rank data for user `{name}`.\n\
                    In order for a target name to work, \
                    the user must be at least top 10k in the score ranking.",
                );

                return orig.error(content).await;
            };

            reach_name = true;

            rank.get()
        }
    };

    if rank > 10_000 {
        let content = "Unfortunately I can only provide data for ranks up to 10,000 :(";

        return orig.error(content).await;
    }

    // Retrieve the user and the user thats holding the given rank
    let rank_holder = if let Some(rank_holder) = rank_holder {
        rank_holder
    } else {
        let page = (rank as usize / 50) + (rank % 50 != 0) as usize;
        let rank_holder_fut = Context::osu().score_rankings(mode).page(page as u32);

        match rank_holder_fut.await {
            Ok(mut rankings) => {
                let idx = (rank as usize + 49) % 50;
                let user_ = rankings.ranking.swap_remove(idx);

                let ranked_score = user_
                    .statistics
                    .as_ref()
                    .map_or(0, |stats| stats.ranked_score);

                // In case the given rank belongs to the user itself,
                // might as well update user data in case it was previously
                // cached.
                let username = if user_.user_id == user.user_id.to_native() {
                    let username = user_.username.clone();
                    user.update(user_);

                    username
                } else {
                    user_.username
                };

                RankHolder {
                    ranked_score,
                    username,
                }
            }
            Err(OsuError::NotFound) => {
                let content = user_not_found(user_id).await;

                return orig.error(content).await;
            }
            Err(err) => {
                let _ = orig.error(OSU_API_ISSUE).await;
                let err = Report::new(err).wrap_err("Failed to get user");

                return Err(err);
            }
        }
    };

    let rank_fut =
        Context::client().get_respektive_users(iter::once(user.user_id.to_native()), mode);

    let respektive_user = match rank_fut.await {
        Ok(mut iter) => iter.next().flatten(),
        Err(err) => {
            warn!(?err, "Failed to get respektive user");

            None
        }
    };

    let username = user.username.as_str().cow_escape_markdown();

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

    let user_score = user
        .statistics
        .as_ref()
        .expect("missing stats")
        .ranked_score
        .to_native();
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
        .thumbnail(user.avatar_url.as_ref().to_owned())
        .title(title);

    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(builder).await?;

    Ok(())
}

struct RankHolder {
    ranked_score: u64,
    username: Username,
}

pub fn author(user: &CachedUser, respektive_user: Option<&RespektiveUser>) -> AuthorBuilder {
    let rank = respektive_user.and_then(|user| user.rank);

    let mut text = format!(
        "{username}: {score}",
        username = user.username.as_str(),
        score = WithComma::new(
            user.statistics
                .as_ref()
                .expect("missing stats")
                .ranked_score
                .to_native()
        ),
    );

    if let Some(rank) = rank {
        let _ = write!(text, " (#{})", WithComma::new(rank.get()));
    }

    let country_code = user.country_code.as_str();
    let user_id = user.user_id.to_native();
    let mode = user.mode;

    let url = format!("{OSU_BASE}users/{user_id}/{mode}");
    let icon = flag_url(country_code);

    AuthorBuilder::new(text).url(url).icon_url(icon)
}
