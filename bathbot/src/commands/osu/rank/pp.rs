use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    iter,
    sync::Arc,
};

use bathbot_macros::command;
use bathbot_model::{
    rosu_v2::{ranking::RankingsUser, user::User},
    Countries,
};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    numbers::WithComma,
    osu::{approx_more_pp, pp_missing, ExtractablePp, PpListUtil},
    CowUtils, EmbedBuilder, MessageBuilder,
};
use eyre::{Report, Result};
use rkyv::{Deserialize, Infallible};
use rosu_v2::prelude::{CountryCode, OsuError, Score};

use super::RankPp;
use crate::{
    commands::{osu::user_not_found, GameModeOption},
    core::commands::{prefix::Args, CommandOrigin},
    manager::redis::{osu::UserArgs, RedisData},
    util::ChannelExt,
    Context,
};

pub(super) async fn pp(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: RankPp<'_>) -> Result<()> {
    let (user_id, mode) = user_id_mode!(ctx, orig, args);

    let RankPp {
        country,
        rank,
        each,
        ..
    } = args;

    let country = match country {
        Some(ref country) => match Countries::name(country).to_code() {
            Some(code) => Some(CountryCode::from(code)),
            None if country.len() == 2 => {
                Some(CountryCode::from(country.cow_to_ascii_uppercase().as_ref()))
            }
            None => {
                let content =
                    format!("Looks like `{country}` is neither a country name nor a country code");

                return orig.error(&ctx, content).await;
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
                    RedisData::Original(mut rankings) => {
                        RankingsUser::from(rankings.ranking.swap_remove(idx))
                    }
                    RedisData::Archive(rankings) => {
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

                return Err(Report::new(err).wrap_err("Failed to get user"));
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

                return Err(Report::new(err).wrap_err("Failed to get user"));
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
                let err = Report::new(err).wrap_err("Failed to get scores");

                return Err(err);
            }
        }
    } else {
        None
    };

    let multiple = each.map_or(RankMultipleScores::Single, RankMultipleScores::EachPp);

    let title = rank_data.title();
    let user = rank_data.user();
    let description = rank_data.description(scores.as_deref(), multiple);

    let embed = EmbedBuilder::new()
        .author(user.author_builder())
        .description(description)
        .thumbnail(user.avatar_url())
        .title(title);

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
                    country = Some(prefix.into());
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

#[derive(Copy, Clone)]
enum RankMultipleScores {
    EachPp(f32),
    Single,
}

enum RankData {
    Sub10k {
        user: RedisData<User>,
        rank: u32,
        country: Option<CountryCode>,
        rank_holder: Box<RankingsUser>,
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
            } => user.stats().pp() < rank_holder.statistics.as_ref().unwrap().pp,
            Self::Over10k {
                user, required_pp, ..
            } => user.stats().pp() < *required_pp,
        }
    }

    fn user(&self) -> &RedisData<User> {
        match self {
            Self::Sub10k { user, .. } => user,
            Self::Over10k { user, .. } => user,
        }
    }

    fn title(&self) -> String {
        match self {
            RankData::Sub10k {
                user,
                rank,
                country,
                ..
            } => {
                let username = user.username().cow_escape_markdown();

                format!(
                    "How many pp is {username} missing to reach rank {country}{rank}?",
                    country = country.as_ref().map(|code| code.as_str()).unwrap_or("#"),
                )
            }
            RankData::Over10k { user, rank, .. } => {
                let username = user.username().cow_escape_markdown();

                format!(
                    "How many pp is {username} missing to reach rank #{rank}?",
                    rank = WithComma::new(*rank),
                )
            }
        }
    }

    fn description(&self, scores: Option<&[Score]>, multiple: RankMultipleScores) -> String {
        match self {
            RankData::Sub10k {
                user,
                rank,
                country,
                rank_holder,
            } => {
                let username = user.username();
                let user_id = user.user_id();
                let user_pp = user.stats().pp();

                let rank_holder_pp = rank_holder
                    .statistics
                    .as_ref()
                    .map_or(0.0, |stats| stats.pp);

                if user_id == rank_holder.user_id {
                    return format!("{username} is already at rank #{rank}.");
                } else if user_pp > rank_holder_pp {
                    return format!(
                        "Rank {rank} is currently held by {holder_name} with \
                        **{holder_pp}pp**, so {username} is already above that with **{pp}pp**.",
                        rank = RankFormat::new(*rank, country.is_none(), rank_holder),
                        holder_name = rank_holder.username.cow_escape_markdown(),
                        holder_pp = WithComma::new(rank_holder_pp),
                        pp = WithComma::new(user_pp)
                    );
                }

                let Some(scores) = scores else {
                    return format!(
                        "Rank {rank} is currently held by {holder_name} with \
                        **{holder_pp}pp**, so {username} is missing **{holder_pp}** raw pp, \
                        achievable with a single score worth **{holder_pp}pp**.",
                        rank = RankFormat::new(*rank, country.is_none(), rank_holder),
                        holder_name = rank_holder.username.cow_escape_markdown(),
                        holder_pp = WithComma::new(rank_holder_pp),
                    );
                };

                match multiple {
                    RankMultipleScores::EachPp(each) => {
                        if let Some(last_pp) = scores.last().and_then(|s| s.pp) {
                            if each < last_pp {
                                return format!(
                                    "Rank {rank} is currently held by {holder_name} with \
                                    **{holder_pp}pp**, so {username} is missing **{missing}** raw pp.\n\
                                    A new top100 score requires at least **{last_pp}pp** \
                                    so {holder_pp} total pp can't be reached with {each}pp scores.",
                                    rank = RankFormat::new(*rank, country.is_none(), rank_holder),
                                    holder_name = rank_holder.username.cow_escape_markdown(),
                                    holder_pp = WithComma::new(rank_holder_pp),
                                    missing = WithComma::new(rank_holder_pp - user_pp),
                                    last_pp = WithComma::new(last_pp),
                                    each = WithComma::new(each),
                                );
                            }
                        }

                        let mut pps = scores.extract_pp();

                        let (required, idx) = if scores.len() == 100 {
                            approx_more_pp(&mut pps, 50);

                            pp_missing(user_pp, rank_holder_pp, pps.as_slice())
                        } else {
                            pp_missing(user_pp, rank_holder_pp, scores)
                        };

                        if required < each {
                            return format!(
                                "Rank {rank} is currently held by {holder_name} with \
                                **{holder_pp}pp**, so {username} is missing **{missing}** raw pp.\n\
                                To reach {holder_pp}pp with one additional score, {username} needs to \
                                perform a **{required}pp** score which would be the top {approx}#{idx}",
                                rank = RankFormat::new(*rank, country.is_none(), rank_holder),
                                holder_name = rank_holder.username.cow_escape_markdown(),
                                holder_pp = WithComma::new(rank_holder_pp),
                                missing = WithComma::new(rank_holder_pp - user_pp),
                                required = WithComma::new(required),
                                approx = if idx >= 100 { "~" } else { "" },
                                idx = idx + 1,
                            );
                        }

                        let idx = pps.iter().position(|&pp| pp < each).unwrap_or(pps.len());

                        let mut iter = pps
                            .iter()
                            .copied()
                            .zip(0..)
                            .map(|(pp, i)| pp * 0.95_f32.powi(i));

                        let mut top: f32 = (&mut iter).take(idx).sum();
                        let bot: f32 = iter.sum();

                        let bonus_pp = (user_pp - (top + bot)).max(0.0);
                        top += bonus_pp;
                        let len = pps.len();

                        let mut n_each = len;

                        for i in idx..len {
                            let bot = pps[idx..]
                                .iter()
                                .copied()
                                .zip(i as i32 + 1..)
                                .fold(0.0, |sum, (pp, i)| sum + pp * 0.95_f32.powi(i));

                            let factor = 0.95_f32.powi(i as i32);

                            if top + factor * each + bot >= rank_holder_pp {
                                // requires n_each many new scores of `each` many pp and one
                                // additional score
                                n_each = i - idx;
                                break;
                            }

                            top += factor * each;
                        }

                        if n_each == len {
                            return format!(
                                "Rank {rank} is currently held by {holder_name} with \
                                **{holder_pp}pp**, so {username} is missing **{missing}** raw pp.\n\
                                Filling up {username}'{genitiv} top scores with {amount} new \
                                {each}pp score{plural} would only lead to {approx}**{top}pp** which \
                                is still less than {holder_pp}pp.",
                                rank = RankFormat::new(*rank, country.is_none(), rank_holder),
                                holder_name = rank_holder.username.cow_escape_markdown(),
                                holder_pp = WithComma::new(rank_holder_pp),
                                amount = len - idx,
                                each = WithComma::new(each),
                                missing = WithComma::new(rank_holder_pp - user_pp),
                                plural = if len - idx != 1 { "s" } else { "" },
                                genitiv = if idx != 1 { "s" } else { "" },
                                approx = if idx >= 100 { "roughly " } else { "" },
                                top = WithComma::new(top),
                            );
                        }

                        pps.extend(iter::repeat(each).take(n_each));

                        pps.sort_unstable_by(|a, b| b.total_cmp(a));

                        let accum = pps.accum_weighted();

                        // Calculate the pp of the missing score after adding `n_each`
                        // many `each` pp scores
                        let total = accum + bonus_pp;
                        let (required, _) = pp_missing(total, rank_holder_pp, pps.as_slice());

                        format!(
                            "Rank {rank} is currently held by {holder_name} with \
                            **{holder_pp}pp**, so {username} is missing **{missing}** raw pp.\n\
                            To reach {holder_pp}pp, {username} needs to perform **{n_each}** \
                            more {each}pp score{plural} and one **{required}pp** score.",
                            rank = RankFormat::new(*rank, country.is_none(), rank_holder),
                            holder_name = rank_holder.username.cow_escape_markdown(),
                            holder_pp = WithComma::new(rank_holder_pp),
                            missing = WithComma::new(rank_holder_pp - user_pp),
                            each = WithComma::new(each),
                            plural = if n_each != 1 { "s" } else { "" },
                            required = WithComma::new(required),
                        )
                    }
                    RankMultipleScores::Single => {
                        let (required, idx) = if scores.len() == 100 {
                            let mut pps = scores.extract_pp();
                            approx_more_pp(&mut pps, 50);

                            pp_missing(user_pp, rank_holder_pp, pps.as_slice())
                        } else {
                            pp_missing(user_pp, rank_holder_pp, scores)
                        };

                        format!(
                            "Rank {rank} is currently held by {holder_name} with \
                            **{holder_pp}pp**, so {username} is missing **{missing}** raw pp, achievable \
                            with a single score worth **{pp}pp** which would be the top {approx}#{idx}.",
                            rank = RankFormat::new(*rank, country.is_none(), rank_holder),
                            holder_name = rank_holder.username.cow_escape_markdown(),
                            holder_pp = WithComma::new(rank_holder_pp),
                            missing = WithComma::new(rank_holder_pp - user_pp),
                            pp = WithComma::new(required),
                            approx = if idx >= 100 { "~" } else { "" },
                            idx = idx + 1,
                        )
                    }
                }
            }
            RankData::Over10k {
                user,
                rank,
                required_pp,
            } => {
                let username = user.username().cow_escape_markdown();
                let user_pp = user.stats().pp();

                if user_pp > *required_pp {
                    return format!(
                        "Rank #{rank} currently requires approx. **{required_pp}pp**, \
                        so {username} is already above that with **{pp}pp**.",
                        rank = WithComma::new(*rank),
                        required_pp = WithComma::new(*required_pp),
                        pp = WithComma::new(user_pp)
                    );
                }

                let Some(scores) = scores else {
                    return format!(
                        "Rank #{rank} currently requires approx. **{required_pp}pp**, \
                        so {username} is missing **{required_pp}** raw pp, \
                        achievable with a single score worth **{required_pp}pp**.",
                        rank = WithComma::new(*rank),
                        required_pp = WithComma::new(*required_pp),
                    );
                };

                match multiple {
                    RankMultipleScores::EachPp(each) => {
                        if let Some(last_pp) = scores.last().and_then(|s| s.pp) {
                            if each < last_pp {
                                return format!(
                                    "Rank #{rank} currently requires approx. **{required_pp}pp**, \
                                    so {username} is missing **{missing}** raw pp.\n\
                                    A new top100 score requires at least **{last_pp}pp** \
                                    so {required_pp} total pp can't be reached with {each}pp scores.",
                                    required_pp = WithComma::new(*required_pp),
                                    missing = WithComma::new(required_pp - user_pp),
                                    last_pp = WithComma::new(last_pp),
                                    each = WithComma::new(each),
                                );
                            }
                        }

                        let mut pps = scores.extract_pp();

                        let (required, idx) = if scores.len() == 100 {
                            approx_more_pp(&mut pps, 50);

                            pp_missing(user_pp, *required_pp, pps.as_slice())
                        } else {
                            pp_missing(user_pp, *required_pp, scores)
                        };

                        if required < each {
                            return format!(
                                "Rank #{rank} currently requires approx. **{required_pp}pp**, \
                                so {username} is missing **{missing}** raw pp.\n\
                                To reach {required_pp}pp with one additional score, {username} needs to \
                                perform a **{required}pp** score which would be the top {approx}#{idx}",
                                required_pp = WithComma::new(*required_pp),
                                missing = WithComma::new(required_pp - user_pp),
                                required = WithComma::new(required),
                                approx = if idx >= 100 { "~" } else { "" },
                                idx = idx + 1,
                            );
                        }

                        let idx = pps.iter().position(|&pp| pp < each).unwrap_or(pps.len());

                        let mut iter = pps
                            .iter()
                            .copied()
                            .zip(0..)
                            .map(|(pp, i)| pp * 0.95_f32.powi(i));

                        let mut top: f32 = (&mut iter).take(idx).sum();
                        let bot: f32 = iter.sum();

                        let bonus_pp = (user_pp - (top + bot)).max(0.0);
                        top += bonus_pp;
                        let len = pps.len();

                        let mut n_each = len;

                        for i in idx..len {
                            let bot = pps[idx..]
                                .iter()
                                .copied()
                                .zip(i as i32 + 1..)
                                .fold(0.0, |sum, (pp, i)| sum + pp * 0.95_f32.powi(i));

                            let factor = 0.95_f32.powi(i as i32);

                            if top + factor * each + bot >= *required_pp {
                                // requires n_each many new scores of `each` many pp and one
                                // additional score
                                n_each = i - idx;
                                break;
                            }

                            top += factor * each;
                        }

                        if n_each == len {
                            return format!(
                                "Rank #{rank} currently requires approx. **{required_pp}pp**, \
                                so {username} is missing **{missing}** raw pp.\n\
                                Filling up {username}'{genitiv} top scores with {amount} new \
                                {each}pp score{plural} would only lead to {approx}**{top}pp** which \
                                is still less than {required_pp}pp.",
                                required_pp = WithComma::new(*required_pp),
                                amount = len - idx,
                                each = WithComma::new(each),
                                missing = WithComma::new(required_pp - user_pp),
                                plural = if len - idx != 1 { "s" } else { "" },
                                genitiv = if idx != 1 { "s" } else { "" },
                                approx = if idx >= 100 { "roughly " } else { "" },
                                top = WithComma::new(top),
                            );
                        }

                        pps.extend(iter::repeat(each).take(n_each));

                        pps.sort_unstable_by(|a, b| b.total_cmp(a));

                        let accum = pps.accum_weighted();

                        // Calculate the pp of the missing score after adding `n_each`
                        // many `each` pp scores
                        let total = accum + bonus_pp;
                        let (required, _) = pp_missing(total, *required_pp, pps.as_slice());

                        format!(
                            "Rank #{rank} currently requires approx. **{required_pp}pp**, \
                            so {username} is missing **{missing}** raw pp.\n\
                            To reach {required_pp}pp, {username} needs to perform **{n_each}** \
                            more {each}pp score{plural} and one **{required}pp** score.",
                            required_pp = WithComma::new(*required_pp),
                            missing = WithComma::new(required_pp - user_pp),
                            each = WithComma::new(each),
                            plural = if n_each != 1 { "s" } else { "" },
                            required = WithComma::new(required),
                        )
                    }
                    RankMultipleScores::Single => {
                        let (required, idx) = if scores.len() == 100 {
                            let mut pps = scores.extract_pp();
                            approx_more_pp(&mut pps, 50);

                            pp_missing(user_pp, *required_pp, pps.as_slice())
                        } else {
                            pp_missing(user_pp, *required_pp, scores)
                        };

                        format!(
                            "Rank #{rank} currently requires approx. **{required_pp}pp**, so \
                            {username} is missing **{missing}** raw pp, achievable with a \
                            single score worth **{pp}pp** which would be the top {approx}#{idx}.",
                            rank = WithComma::new(*rank),
                            required_pp = WithComma::new(*required_pp),
                            missing = WithComma::new(required_pp - user_pp),
                            pp = WithComma::new(required),
                            approx = if idx >= 100 { "~" } else { "" },
                            idx = idx + 1,
                        )
                    }
                }
            }
        }
    }
}

struct RankFormat<'d> {
    rank: u32,
    global: bool,
    holder: &'d RankingsUser,
}

impl<'d> RankFormat<'d> {
    fn new(rank: u32, global: bool, holder: &'d RankingsUser) -> Self {
        Self {
            rank,
            global,
            holder,
        }
    }
}

impl Display for RankFormat<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.global {
            return write!(f, "#{}", self.rank);
        }

        write!(f, "{}{}", self.holder.country_code, self.rank)?;

        let global_rank = self
            .holder
            .statistics
            .as_ref()
            .map(|stats| stats.global_rank);

        if let Some(global_rank) = global_rank {
            write!(f, " (#{global_rank})")?;
        }

        Ok(())
    }
}
