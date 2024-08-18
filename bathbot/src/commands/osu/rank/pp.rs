use std::{
    borrow::Cow,
    cmp,
    convert::identity,
    fmt::{Display, Formatter, Result as FmtResult},
    iter,
};

use bathbot_macros::command;
use bathbot_model::{command_fields::GameModeOption, rosu_v2::user::User, Countries};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    numbers::WithComma,
    osu::{approx_more_pp, pp_missing, ExtractablePp, PpListUtil},
    CowUtils, EmbedBuilder, MessageBuilder,
};
use eyre::{Report, Result};
use rosu_v2::prelude::{CountryCode, OsuError, Score, UserId, Username};

use super::{RankPp, RankValue};
use crate::{
    commands::osu::user_not_found,
    core::commands::{prefix::Args, CommandOrigin},
    embeds::PersonalBestIndexFormatter,
    manager::redis::{
        osu::{UserArgs, UserArgsSlim},
        RedisData,
    },
    util::ChannelExt,
    Context,
};

pub(super) async fn pp(orig: CommandOrigin<'_>, args: RankPp<'_>) -> Result<()> {
    let (user_id, mode) = user_id_mode!(orig, args);

    let RankPp {
        country,
        rank,
        each,
        amount,
        ..
    } = args;

    let rank_value = RankValue::parse(rank.as_ref());

    let country = match country {
        Some(ref country) => match Countries::name(country).to_code() {
            Some(code) => Some(CountryCode::from(code)),
            None if country.len() == 2 => {
                Some(CountryCode::from(country.cow_to_ascii_uppercase().as_ref()))
            }
            None => {
                let content =
                    format!("Looks like `{country}` is neither a country name nor a country code");

                return orig.error(content).await;
            }
        },
        None => None,
    };

    if matches!(rank_value, RankValue::Raw(0)) {
        return orig.error("Rank can't be zero :clown:").await;
    } else if matches!(rank_value, RankValue::Delta(0)) {
        return orig.error("Delta must be greater than zero :clown:").await;
    }

    let user_args = UserArgs::rosu_id(&user_id, mode).await;
    let user_fut = Context::redis().osu_user(user_args);

    let mut user = match user_fut.await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(OSU_API_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get user"));
        }
    };

    let rank_or_holder = match rank_value {
        RankValue::Delta(delta) => RankOrHolder::Rank(cmp::max(
            1,
            user.stats().global_rank().saturating_sub(delta),
        )),
        RankValue::Raw(rank) => RankOrHolder::Rank(rank),
        RankValue::Name(name) => {
            let user_id = UserId::from(name);
            let user_args = UserArgs::rosu_id(&user_id, mode).await;

            match Context::redis().osu_user(user_args).await {
                Ok(target_user) => {
                    let rank_holder = RankHolder {
                        country_code: target_user.country_code().into(),
                        global_rank: target_user.stats().global_rank(),
                        pp: target_user.stats().pp(),
                        user_id: target_user.user_id(),
                        username: target_user.username().into(),
                    };

                    RankOrHolder::Holder(rank_holder)
                }
                Err(OsuError::NotFound) => {
                    let content = user_not_found(user_id).await;

                    return orig.error(content).await;
                }
                Err(err) => {
                    let _ = orig.error(OSU_API_ISSUE).await;

                    return Err(Report::new(err).wrap_err("Failed to get target user"));
                }
            }
        }
    };

    let rank = rank_or_holder.rank();

    if rank_or_holder.rank() > 10_000 && country.is_some() {
        let content = "Unfortunately I can only provide data for country ranks up to 10,000 :(";

        return orig.error(content).await;
    }

    async fn insufficient_ranking_entries(orig: CommandOrigin<'_>) -> Result<()> {
        orig.error("Not enough ranking entries available").await
    }

    let rank_data = match rank_or_holder {
        RankOrHolder::Rank(rank) if rank <= 10_000 => {
            // Retrieve the user and the user thats holding the given rank
            let page = (rank / 50) + (rank % 50 != 0) as u32;

            let rankings_fut =
                Context::redis().pp_ranking(mode, page, country.as_ref().map(|c| c.as_str()));

            let rankings = match rankings_fut.await {
                Ok(rankings) => rankings,
                Err(OsuError::NotFound) => {
                    let content = user_not_found(user_id).await;

                    return orig.error(content).await;
                }
                Err(err) => {
                    let _ = orig.error(OSU_API_ISSUE).await;

                    return Err(Report::new(err).wrap_err("Failed to get user"));
                }
            };

            let idx = ((rank + 49) % 50) as usize;

            let rank_holder = match rankings {
                RedisData::Original(mut rankings) => {
                    if rankings.ranking.len() <= idx {
                        return insufficient_ranking_entries(orig).await;
                    }

                    let holder = rankings.ranking.swap_remove(idx);

                    let global_rank = holder
                        .statistics
                        .as_ref()
                        .and_then(|stats| stats.global_rank)
                        .unwrap_or(0);

                    let pp = holder.statistics.as_ref().map_or(0.0, |stats| stats.pp);
                    let user_id = holder.user_id;

                    let (username, country_code) = if user_id == user.user_id() {
                        let tuple = (holder.username.clone(), holder.country_code.clone());
                        user.update(holder);

                        tuple
                    } else {
                        (holder.username, holder.country_code)
                    };

                    RankHolder {
                        country_code,
                        global_rank,
                        pp,
                        user_id,
                        username,
                    }
                }
                RedisData::Archive(rankings) => {
                    if rankings.ranking.len() <= idx {
                        return insufficient_ranking_entries(orig).await;
                    }

                    let holder = &rankings.ranking[idx];

                    RankHolder {
                        country_code: holder.country_code.as_str().into(),
                        global_rank: holder
                            .statistics
                            .as_ref()
                            .map_or(0, |stats| stats.global_rank),
                        pp: holder.statistics.as_ref().map_or(0.0, |stats| stats.pp),
                        user_id: holder.user_id,
                        username: holder.username.as_str().into(),
                    }
                }
            };

            RankData::Sub10k {
                user,
                rank,
                country,
                rank_holder,
            }
        }
        RankOrHolder::Rank(rank) => {
            let required_pp = match Context::approx().pp(rank, mode).await {
                Ok(pp) => pp,
                Err(err) => {
                    let _ = orig.error(GENERAL_ISSUE).await;

                    return Err(err);
                }
            };

            RankData::Over10kApprox {
                user,
                rank,
                required_pp,
            }
        }
        RankOrHolder::Holder(rank_holder) if rank <= 10_000 => {
            RankData::Sub10kExact { user, rank_holder }
        }
        RankOrHolder::Holder(rank_holder) => RankData::Over10kExact { user, rank_holder },
    };

    // Retrieve the user's top scores if required
    let scores = if rank_data.with_scores() {
        let user = rank_data.user();

        let user_args = UserArgsSlim::user_id(user.user_id()).mode(mode);
        let scores_fut = Context::osu_scores()
            // making sure to retrieve potential lazer top scores as well;
            // no need to retrieve it via user/guild config
            .top(false)
            .limit(100)
            .exec(user_args);

        match scores_fut.await {
            Ok(scores) => (!scores.is_empty()).then_some(scores),
            Err(err) => {
                let _ = orig.error(OSU_API_ISSUE).await;
                let err = Report::new(err).wrap_err("Failed to get scores");

                return Err(err);
            }
        }
    } else {
        None
    };

    let multiple = match (each, amount) {
        (None, None) => RankMultipleScores::Amount(1),
        (Some(each), _) => RankMultipleScores::EachPp(each),
        (None, Some(amount)) => RankMultipleScores::Amount(amount),
    };

    let title = rank_data.title();
    let user = rank_data.user();
    let description = rank_data.description(scores.as_deref(), multiple);

    let embed = EmbedBuilder::new()
        .author(user.author_builder())
        .description(description)
        .thumbnail(user.avatar_url())
        .title(title);

    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(builder).await?;

    Ok(())
}

#[command]
#[desc("How many pp is a player missing to reach the given rank?")]
#[help(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the value is an approximation based on cached user data.\n\
    If no number is given, one of the arguments will be considered as username whose rank should be reached.\n\
    To make sure the correct target input is used you can prefix it with `rank=` e.g. `rank=123` or `rank=mrekk`."
)]
#[usage("[username] [[country]number/username]")]
#[examples("badewanne3 be50", "badewanne3 123")]
#[alias("reach")]
#[group(Osu)]
async fn prefix_rank(msg: &Message, args: Args<'_>) -> Result<()> {
    match RankPp::args(None, args) {
        Ok(args) => pp(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many pp is a player missing to reach the given rank?")]
#[help(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the value is an approximation based on cached user data.\n\
    If no number is given, one of the arguments will be considered as username whose rank should be reached.\n\
    To make sure the correct target input is used you can prefix it with `rank=` e.g. `rank=123` or `rank=mrekk`."
)]
#[usage("[username] [[country]number/username]")]
#[examples("badewanne3 be50", "badewanne3 123")]
#[alias("rankm", "reachmania", "reachm")]
#[group(Mania)]
async fn prefix_rankmania(msg: &Message, args: Args<'_>) -> Result<()> {
    match RankPp::args(Some(GameModeOption::Mania), args) {
        Ok(args) => pp(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many pp is a player missing to reach the given rank?")]
#[help(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the value is an approximation based on cached user data.\n\
    If no number is given, one of the arguments will be considered as username whose rank should be reached.\n\
    To make sure the correct target input is used you can prefix it with `rank=` e.g. `rank=123` or `rank=mrekk`."
)]
#[usage("[username] [[country]number/username]")]
#[examples("badewanne3 be50", "badewanne3 123")]
#[alias("rankt", "reachtaiko", "reacht")]
#[group(Taiko)]
async fn prefix_ranktaiko(msg: &Message, args: Args<'_>) -> Result<()> {
    match RankPp::args(Some(GameModeOption::Taiko), args) {
        Ok(args) => pp(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many pp is a player missing to reach the given rank?")]
#[help(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the value is an approximation based on cached user data.\n\
    If no number is given, one of the arguments will be considered as username whose rank should be reached.\n\
    To make sure the correct target input is used you can prefix it with `rank=` e.g. `rank=123` or `rank=mrekk`."
)]
#[usage("[username] [[country]number/username]")]
#[examples("badewanne3 be50", "badewanne3 123")]
#[alias("rankc", "reachctb", "reachc", "rankcatch", "reachcatch")]
#[group(Catch)]
async fn prefix_rankctb(msg: &Message, args: Args<'_>) -> Result<()> {
    match RankPp::args(Some(GameModeOption::Catch), args) {
        Ok(args) => pp(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

impl<'m> RankPp<'m> {
    fn args(mode: Option<GameModeOption>, mut args: Args<'m>) -> Result<Self, &'static str> {
        enum Prefixed<'a> {
            Rank {
                value: &'a str,
                country_code: Option<Cow<'a, str>>,
                prefixed: bool,
            },
            Name(&'a str),
            None,
        }

        impl<'a> Prefixed<'a> {
            fn parse(arg: &'a str) -> Prefixed<'a> {
                let Some((key, value)) = arg.split_once('=') else {
                    return Self::parse_rank(arg, false).unwrap_or(Self::None);
                };

                match key {
                    "rank" | "reach" | "r" => Self::parse_rank(value, true).unwrap_or(Self::Rank {
                        value,
                        country_code: None,
                        prefixed: true,
                    }),
                    "user" | "u" | "name" | "n" => Self::Name(value),
                    _ => Self::None,
                }
            }

            fn parse_rank(arg: &'a str, prefixed: bool) -> Option<Prefixed<'a>> {
                if arg.parse::<u32>().is_ok() {
                    return Some(Self::Rank {
                        value: arg,
                        country_code: None,
                        prefixed,
                    });
                }

                let mut chars = arg.chars();

                let valid_country = chars.by_ref().take(2).all(|c| c.is_ascii_alphabetic());

                // at least one digit and all following must be digits too
                let valid_rank = chars.next().is_some_and(|c| c.is_ascii_digit())
                    && chars.all(|c| c.is_ascii_digit());

                if valid_country && valid_rank {
                    let (country, rank) = arg.split_at(2);

                    Some(Self::Rank {
                        value: rank,
                        country_code: Some(Cow::Borrowed(country)),
                        prefixed,
                    })
                } else {
                    None
                }
            }
        }

        let mut name = None;
        let mut country = None;
        let mut rank = None;
        let mut discord = None;

        if let Some(first) = args.next() {
            if let Some(second) = args.next() {
                match (Prefixed::parse(first), Prefixed::parse(second)) {
                    (
                        Prefixed::Rank {
                            value,
                            country_code,
                            prefixed: true,
                        },
                        Prefixed::Rank { .. },
                    )
                    | (
                        Prefixed::Rank {
                            value,
                            country_code,
                            prefixed: false,
                        },
                        Prefixed::Rank {
                            prefixed: false, ..
                        },
                    )
                    | (
                        Prefixed::Rank {
                            value,
                            country_code,
                            prefixed: _,
                        },
                        Prefixed::None,
                    ) => {
                        rank = Some(value);
                        country = country_code;
                        name = Some(Cow::Borrowed(second));
                    }
                    (
                        Prefixed::Rank {
                            prefixed: false, ..
                        },
                        Prefixed::Rank {
                            value,
                            country_code,
                            prefixed: true,
                        },
                    ) => {
                        rank = Some(value);
                        country = country_code;
                        name = Some(Cow::Borrowed(first));
                    }
                    (
                        Prefixed::Rank {
                            value,
                            country_code,
                            prefixed: _,
                        },
                        Prefixed::Name(name_value),
                    ) => {
                        rank = Some(value);
                        country = country_code;
                        name = Some(Cow::Borrowed(name_value));
                    }
                    (
                        Prefixed::None,
                        Prefixed::Rank {
                            value,
                            country_code,
                            prefixed: _,
                        },
                    ) => {
                        rank = Some(value);
                        country = country_code;
                        name = Some(Cow::Borrowed(first));
                    }
                    (
                        Prefixed::Name(name_value),
                        Prefixed::Rank {
                            value,
                            country_code,
                            prefixed: _,
                        },
                    ) => {
                        rank = Some(value);
                        country = country_code;
                        name = Some(Cow::Borrowed(name_value));
                    }
                    (Prefixed::Name(rank_value), Prefixed::Name(name_value)) => {
                        rank = Some(rank_value);
                        name = Some(Cow::Borrowed(name_value));
                    }
                    (Prefixed::Name(value), Prefixed::None) => {
                        rank = Some(second);
                        name = Some(Cow::Borrowed(value));
                    }
                    (Prefixed::None, Prefixed::Name(value)) => {
                        rank = Some(first);
                        name = Some(Cow::Borrowed(value));
                    }
                    (Prefixed::None, Prefixed::None) => {
                        rank = Some(first);
                        name = Some(Cow::Borrowed(second));
                    }
                }
            } else {
                match Prefixed::parse(first) {
                    Prefixed::Rank {
                        value,
                        country_code,
                        prefixed: _,
                    } => {
                        rank = Some(value);
                        country = country_code;
                    }
                    Prefixed::Name(name_value) => name = Some(Cow::Borrowed(name_value)),
                    Prefixed::None => name = Some(Cow::Borrowed(first)),
                }
            }
        }

        if let Some(ref name_value) = name {
            if let Some(id) = matcher::get_mention_user(name_value) {
                discord = Some(id);
                name = None;
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
            each: None,
            amount: None,
            country,
            discord,
        })
    }
}

#[derive(Copy, Clone)]
enum RankMultipleScores {
    Amount(u8),
    EachPp(f32),
}

enum RankData {
    Sub10k {
        user: RedisData<User>,
        rank: u32,
        country: Option<CountryCode>,
        rank_holder: RankHolder,
    },
    Sub10kExact {
        user: RedisData<User>,
        rank_holder: RankHolder,
    },
    Over10kApprox {
        user: RedisData<User>,
        rank: u32,
        required_pp: f32,
    },
    Over10kExact {
        user: RedisData<User>,
        rank_holder: RankHolder,
    },
}

struct RankHolder {
    country_code: CountryCode,
    global_rank: u32,
    pp: f32,
    user_id: u32,
    username: Username,
}

fn idx_suffix(idx: usize) -> &'static str {
    match idx % 100 {
        11 | 12 | 13 => "th",
        _ => match idx % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    }
}

impl RankData {
    fn with_scores(&self) -> bool {
        match self {
            Self::Sub10k {
                user, rank_holder, ..
            } => user.stats().pp() < rank_holder.pp,
            Self::Sub10kExact { user, rank_holder } => user.stats().pp() < rank_holder.pp,
            Self::Over10kApprox {
                user, required_pp, ..
            } => user.stats().pp() < *required_pp,
            Self::Over10kExact { user, rank_holder } => user.stats().pp() < rank_holder.pp,
        }
    }

    fn user(&self) -> &RedisData<User> {
        match self {
            Self::Sub10k { user, .. } => user,
            Self::Sub10kExact { user, .. } => user,
            Self::Over10kApprox { user, .. } => user,
            Self::Over10kExact { user, .. } => user,
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
                format!(
                    "How many pp is {username} missing to reach rank {country}{rank}?",
                    username = user.username().cow_escape_markdown(),
                    country = country.as_ref().map(|code| code.as_str()).unwrap_or("#"),
                )
            }
            RankData::Sub10kExact { user, rank_holder } => {
                let holder_name = rank_holder.username.as_str();

                format!(
                    "How many pp is {username} missing to reach \
                    {holder_name}'{genitiv} rank #{rank}?",
                    username = user.username().cow_escape_markdown(),
                    genitiv = if holder_name.ends_with('s') { "" } else { "s" },
                    rank = rank_holder.global_rank,
                )
            }
            RankData::Over10kExact { user, rank_holder } => {
                let holder_name = rank_holder.username.cow_escape_markdown();

                format!(
                    "How many pp is {username} missing to reach \
                    {holder_name}'{genitiv} rank #{rank}?",
                    username = user.username().cow_escape_markdown(),
                    genitiv = if holder_name.ends_with('s') { "" } else { "s" },
                    rank = WithComma::new(rank_holder.global_rank),
                )
            }
            RankData::Over10kApprox { user, rank, .. } => {
                format!(
                    "How many pp is {username} missing to reach rank #{rank}?",
                    username = user.username().cow_escape_markdown(),
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
                let prefix = format!(
                    "Rank {rank} is currently held by {name} with **{pp}pp**",
                    name = rank_holder.username.cow_escape_markdown(),
                    rank = RankFormat::new(*rank, country.is_none(), rank_holder),
                    pp = WithComma::new(rank_holder.pp),
                );

                Self::description_sub_10k(user, &prefix, rank_holder, scores, multiple)
            }
            RankData::Sub10kExact { user, rank_holder } => {
                let prefix = format!(
                    "{name} is rank {rank} with **{pp}pp**",
                    name = rank_holder.username.cow_escape_markdown(),
                    rank = RankFormat::new(rank_holder.global_rank, true, rank_holder),
                    pp = WithComma::new(rank_holder.pp),
                );

                Self::description_sub_10k(user, &prefix, rank_holder, scores, multiple)
            }
            RankData::Over10kApprox {
                user,
                rank,
                required_pp,
            } => Self::description_over_10k(
                user,
                "Rank",
                "approx. ",
                *required_pp,
                *rank,
                scores,
                multiple,
            ),
            RankData::Over10kExact { user, rank_holder } => {
                let holder_name = rank_holder.username.as_str();

                let prefix = format!(
                    "Reaching {holder_name}'{genitiv} rank",
                    holder_name = holder_name.cow_escape_markdown(),
                    genitiv = if holder_name.ends_with('s') { "" } else { "s" }
                );

                Self::description_over_10k(
                    user,
                    &prefix,
                    "",
                    rank_holder.pp,
                    rank_holder.global_rank,
                    scores,
                    multiple,
                )
            }
        }
    }

    fn description_sub_10k(
        user: &RedisData<User>,
        prefix: &str,
        rank_holder: &RankHolder,
        scores: Option<&[Score]>,
        multiple: RankMultipleScores,
    ) -> String {
        let username = user.username().cow_escape_markdown();
        let user_id = user.user_id();
        let user_pp = user.stats().pp();
        let rank = rank_holder.global_rank;
        let rank_holder_pp = rank_holder.pp;

        if user_id == rank_holder.user_id {
            return format!("{username} is already at rank #{rank}.");
        } else if user_pp > rank_holder_pp {
            return format!(
                "{prefix}, so {username} is already above that with **{pp}pp**.",
                pp = WithComma::new(user_pp)
            );
        }

        let Some(scores) = scores else {
            return format!(
                "{prefix}, so {username} is missing **{holder_pp}** raw pp, \
                achievable with a single score worth **{holder_pp}pp**.",
                holder_pp = WithComma::new(rank_holder_pp),
            );
        };

        match multiple {
            RankMultipleScores::Amount(1) => {
                let (required, idx) = if scores.len() == 100 {
                    let mut pps = scores.extract_pp();
                    approx_more_pp(&mut pps, 50);

                    let (mut required, mut idx) =
                        pp_missing(user_pp, rank_holder_pp, pps.as_slice());

                    // Instead of using the approximation too literally, max
                    // out on the 100th top score.
                    let top100 = pps[99];

                    if top100 > required {
                        required = top100;
                        idx = 99;
                    }

                    (required, idx)
                } else {
                    pp_missing(user_pp, rank_holder_pp, scores)
                };

                let idx = idx + 1;

                let suffix = idx_suffix(idx);

                format!(
                    "{prefix}, so {username} is missing **{missing}** raw pp, achievable \
                    with a single score worth **{pp}pp** which would be their {idx}{suffix} top play.",
                    missing = WithComma::new(rank_holder_pp - user_pp),
                    pp = WithComma::new(required),
                )
            }
            RankMultipleScores::Amount(amount) => {
                let mut pps = scores.extract_pp();

                if scores.len() == 100 {
                    approx_more_pp(&mut pps, 50);
                }

                let raw_delta = rank_holder_pp - user_pp;
                let weight_sum: f32 = (0..amount as i32).map(|exp| 0.95_f32.powi(exp)).sum();
                let mid_goal = user_pp + (raw_delta / weight_sum);
                let (mut required, _) = pp_missing(user_pp, mid_goal, pps.as_slice());

                let pb_start_idx = pps
                    .binary_search_by(|probe| required.total_cmp(probe))
                    .map_or_else(identity, |idx| idx + 1);

                let pb_fmt = PersonalBestIndexFormatter::new(pb_start_idx, amount);

                if scores.len() == 100 && required < pps[99] {
                    required = (pps[99] - 0.01).max(0.0);
                }

                format!(
                    "{prefix}, so {username} is missing **{missing}** raw pp. \
                    To catch up with {amount} scores, each one must be worth \
                    **{pp}pp**, placing them {pb_fmt}.",
                    missing = WithComma::new(rank_holder_pp - user_pp),
                    pp = WithComma::new(required),
                )
            }
            RankMultipleScores::EachPp(each) => {
                if let Some(last_pp) = scores.last().and_then(|s| s.pp) {
                    if each < last_pp {
                        return format!(
                            "{prefix}, so {username} is missing **{missing}** raw pp.\n\
                            A new top100 score requires at least **{last_pp}pp** \
                            so {holder_pp} total pp can't be reached with {each}pp scores.",
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
                    let suffix = idx_suffix(idx + 1);

                    return format!(
                        "{prefix}, so {username} is missing **{missing}** raw pp.\n\
                        To reach {holder_pp}pp with one additional score, {username} needs to \
                        perform a **{required}pp** score which would be their {approx}{idx}{suffix} top play",
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
                        "{prefix}, so {username} is missing **{missing}** raw pp.\n\
                        Filling up {username}'{genitiv} top scores with {amount} new \
                        {each}pp score{plural} would only lead to {approx}**{top}pp** which \
                        is still less than {holder_pp}pp.",
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
                    "{prefix}, so {username} is missing **{missing}** raw pp.\n\
                    To reach {holder_pp}pp, {username} needs to perform **{n_each}** \
                    more {each}pp score{plural} and one **{required}pp** score.",
                    holder_pp = WithComma::new(rank_holder_pp),
                    missing = WithComma::new(rank_holder_pp - user_pp),
                    each = WithComma::new(each),
                    plural = if n_each != 1 { "s" } else { "" },
                    required = WithComma::new(required),
                )
            }
        }
    }

    fn description_over_10k(
        user: &RedisData<User>,
        prefix: &str,
        maybe_approx: &str,
        required_pp: f32,
        rank: u32,
        scores: Option<&[Score]>,
        multiple: RankMultipleScores,
    ) -> String {
        let username = user.username().cow_escape_markdown();
        let user_pp = user.stats().pp();

        if user_pp > required_pp {
            return format!(
                "{prefix} #{rank} currently requires {maybe_approx}**{required_pp}pp**, \
                so {username} is already above that with **{pp}pp**.",
                rank = WithComma::new(rank),
                required_pp = WithComma::new(required_pp),
                pp = WithComma::new(user_pp)
            );
        }

        let Some(scores) = scores else {
            return format!(
                "{prefix} #{rank} currently requires {maybe_approx}**{required_pp}pp**, \
                so {username} is missing **{required_pp}** raw pp, \
                achievable with a single score worth **{required_pp}pp**.",
                rank = WithComma::new(rank),
                required_pp = WithComma::new(required_pp),
            );
        };

        match multiple {
            RankMultipleScores::Amount(1) => {
                let (required, idx) = if scores.len() == 100 {
                    let mut pps = scores.extract_pp();
                    approx_more_pp(&mut pps, 50);

                    let (mut required, mut idx) = pp_missing(user_pp, required_pp, pps.as_slice());

                    // Instead of using the approximation too literally, max
                    // out on the 100th top score.
                    let top100 = pps[99];

                    if top100 > required {
                        required = top100;
                        idx = 99;
                    }

                    (required, idx)
                } else {
                    pp_missing(user_pp, required_pp, scores)
                };

                let idx = idx + 1;

                let suffix = idx_suffix(idx);

                format!(
                    "{prefix} #{rank} currently requires {maybe_approx}**{required_pp}pp**, so \
                    {username} is missing **{missing}** raw pp, achievable with a \
                    single score worth **{pp}pp** which would be their {idx}{suffix} top play.",
                    rank = WithComma::new(rank),
                    required_pp = WithComma::new(required_pp),
                    missing = WithComma::new(required_pp - user_pp),
                    pp = WithComma::new(required),
                )
            }
            RankMultipleScores::Amount(amount) => {
                let mut pps = scores.extract_pp();

                if scores.len() == 100 {
                    approx_more_pp(&mut pps, 50);
                }

                let raw_delta = required_pp - user_pp;
                let weight_sum: f32 = (0..amount as i32).map(|exp| 0.95_f32.powi(exp)).sum();
                let mid_goal = user_pp + (raw_delta / weight_sum);
                let (mut required, _) = pp_missing(user_pp, mid_goal, pps.as_slice());

                let pb_start_idx = pps
                    .binary_search_by(|probe| required.total_cmp(probe))
                    .map_or_else(identity, |idx| idx + 1);

                let pb_fmt = PersonalBestIndexFormatter::new(pb_start_idx, amount);

                if scores.len() == 100 && required < pps[99] {
                    required = (pps[99] - 0.01).max(0.0);
                }

                format!(
                    "{prefix} #{rank} currently requires {maybe_approx}**{required_pp}pp**, so \
                    {username} is missing **{missing}** raw pp. To catch up \
                    with {amount} scores, each one must be worth **{pp}pp**, \
                    placing them {pb_fmt}.",
                    rank = WithComma::new(rank),
                    required_pp = WithComma::new(required_pp),
                    missing = WithComma::new(required_pp - user_pp),
                    pp = WithComma::new(required),
                )
            }
            RankMultipleScores::EachPp(each) => {
                if let Some(last_pp) = scores.last().and_then(|s| s.pp) {
                    if each < last_pp {
                        return format!(
                            "{prefix} #{rank} currently requires {maybe_approx}**{required_pp}pp**, \
                            so {username} is missing **{missing}** raw pp.\n\
                            A new top100 score requires at least **{last_pp}pp** \
                            so {required_pp} total pp can't be reached with {each}pp scores.",
                            required_pp = WithComma::new(required_pp),
                            missing = WithComma::new(required_pp - user_pp),
                            last_pp = WithComma::new(last_pp),
                            each = WithComma::new(each),
                        );
                    }
                }

                let mut pps = scores.extract_pp();

                let (required, idx) = if scores.len() == 100 {
                    approx_more_pp(&mut pps, 50);

                    pp_missing(user_pp, required_pp, pps.as_slice())
                } else {
                    pp_missing(user_pp, required_pp, scores)
                };

                if required < each {
                    let suffix = idx_suffix(idx + 1);

                    return format!(
                        "{prefix} #{rank} currently requires {maybe_approx}**{required_pp}pp**, \
                        so {username} is missing **{missing}** raw pp.\n\
                        To reach {required_pp}pp with one additional score, {username} needs to \
                        perform a **{required}pp** score which would be their {approx}{idx}{suffix} top play",
                        required_pp = WithComma::new(required_pp),
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

                    if top + factor * each + bot >= required_pp {
                        // requires n_each many new scores of `each` many pp and one
                        // additional score
                        n_each = i - idx;
                        break;
                    }

                    top += factor * each;
                }

                if n_each == len {
                    return format!(
                        "{prefix} #{rank} currently requires {maybe_approx}**{required_pp}pp**, \
                        so {username} is missing **{missing}** raw pp.\n\
                        Filling up {username}'{genitiv} top scores with {amount} new \
                        {each}pp score{plural} would only lead to {approx}**{top}pp** which \
                        is still less than {required_pp}pp.",
                        required_pp = WithComma::new(required_pp),
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
                let (required, _) = pp_missing(total, required_pp, pps.as_slice());

                format!(
                    "{prefix} #{rank} currently requires {maybe_approx}**{required_pp}pp**, \
                    so {username} is missing **{missing}** raw pp.\n\
                    To reach {required_pp}pp, {username} needs to perform **{n_each}** \
                    more {each}pp score{plural} and one **{required}pp** score.",
                    required_pp = WithComma::new(required_pp),
                    missing = WithComma::new(required_pp - user_pp),
                    each = WithComma::new(each),
                    plural = if n_each != 1 { "s" } else { "" },
                    required = WithComma::new(required),
                )
            }
        }
    }
}

struct RankFormat<'d> {
    rank: u32,
    global: bool,
    holder: &'d RankHolder,
}

impl<'d> RankFormat<'d> {
    fn new(rank: u32, global: bool, holder: &'d RankHolder) -> Self {
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
            write!(f, "#{}", self.rank)
        } else {
            write!(
                f,
                "{}{} (#{})",
                self.holder.country_code, self.rank, self.holder.global_rank
            )
        }
    }
}
enum RankOrHolder {
    Rank(u32),
    Holder(RankHolder),
}

impl RankOrHolder {
    fn rank(&self) -> u32 {
        match self {
            RankOrHolder::Rank(rank) => *rank,
            RankOrHolder::Holder(holder) => holder.global_rank,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::commands::prefix::ArgsNum;

    #[test]
    fn only_rank() {
        let args = Args::new("123", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert!(args.name.is_none());
        assert!(args.country.is_none());
    }

    #[test]
    fn only_prefixed_rank() {
        let args = Args::new("rank=123", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert!(args.name.is_none());
        assert!(args.country.is_none());
    }

    #[test]
    fn only_country_rank() {
        let args = Args::new("be123", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert!(args.name.is_none());
        assert_eq!(args.country.as_deref(), Some("be"));
    }

    #[test]
    fn only_prefixed_country_rank() {
        let args = Args::new("rank=be123", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert!(args.name.is_none());
        assert_eq!(args.country.as_deref(), Some("be"));
    }

    #[test]
    fn two_names() {
        let args = Args::new("peppy smoogi", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "peppy");
        assert_eq!(args.name.as_deref(), Some("smoogi"));
        assert!(args.country.is_none());
    }

    #[test]
    fn two_names_first_prefixed() {
        let args = Args::new("user=cd36 peppy", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "peppy");
        assert_eq!(args.name.as_deref(), Some("cd36"));
        assert!(args.country.is_none());
    }

    #[test]
    fn two_names_second_prefixed() {
        let args = Args::new("peppy user=cd36", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "peppy");
        assert_eq!(args.name.as_deref(), Some("cd36"));
        assert!(args.country.is_none());
    }

    #[test]
    fn two_names_both_prefixed() {
        let args = Args::new("user=peppy user=cd36", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "peppy");
        assert_eq!(args.name.as_deref(), Some("cd36"));
        assert!(args.country.is_none());
    }

    #[test]
    fn rank_name() {
        let args = Args::new("123 peppy", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert_eq!(args.name.as_deref(), Some("peppy"));
        assert!(args.country.is_none());
    }

    #[test]
    fn country_rank_name() {
        let args = Args::new("cd36 peppy", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "36");
        assert_eq!(args.name.as_deref(), Some("peppy"));
        assert_eq!(args.country.as_deref(), Some("cd"));
    }

    #[test]
    fn prefixed_rank_name() {
        let args = Args::new("rank=123 peppy", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert_eq!(args.name.as_deref(), Some("peppy"));
        assert!(args.country.is_none());
    }

    #[test]
    fn prefixed_country_rank_name() {
        let args = Args::new("rank=cd36 peppy", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "36");
        assert_eq!(args.name.as_deref(), Some("peppy"));
        assert_eq!(args.country.as_deref(), Some("cd"));
    }

    #[test]
    fn rank_prefixed_name() {
        let args = Args::new("123 user=cd36", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert_eq!(args.name.as_deref(), Some("cd36"));
        assert!(args.country.is_none());
    }

    #[test]
    fn country_rank_prefixed_name() {
        let args = Args::new("cd36 user=peppy", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "36");
        assert_eq!(args.name.as_deref(), Some("peppy"));
        assert_eq!(args.country.as_deref(), Some("cd"));
    }

    #[test]
    fn prefixed_rank_prefixed_name() {
        let args = Args::new("rank=123 user=peppy", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert_eq!(args.name.as_deref(), Some("peppy"));
        assert!(args.country.is_none());
    }

    #[test]
    fn prefixed_country_rank_prefixed_name() {
        let args = Args::new("rank=cd36 user=peppy", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "36");
        assert_eq!(args.name.as_deref(), Some("peppy"));
        assert_eq!(args.country.as_deref(), Some("cd"));
    }

    #[test]
    fn name_rank() {
        let args = Args::new("peppy 123", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert_eq!(args.name.as_deref(), Some("peppy"));
        assert!(args.country.is_none());
    }

    #[test]
    fn name_country_rank() {
        let args = Args::new("peppy be123", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert_eq!(args.name.as_deref(), Some("peppy"));
        assert_eq!(args.country.as_deref(), Some("be"));
    }

    #[test]
    fn name_prefixed_rank() {
        let args = Args::new("peppy rank=123", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert_eq!(args.name.as_deref(), Some("peppy"));
        assert!(args.country.is_none());
    }

    #[test]
    fn name_prefixed_country_rank() {
        let args = Args::new("peppy rank=be123", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert_eq!(args.name.as_deref(), Some("peppy"));
        assert_eq!(args.country.as_deref(), Some("be"));
    }

    #[test]
    fn prefixed_name_rank() {
        let args = Args::new("user=cd36 123", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert_eq!(args.name.as_deref(), Some("cd36"));
        assert!(args.country.is_none());
    }

    #[test]
    fn prefixed_name_country_rank() {
        let args = Args::new("user=cd36 be123", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert_eq!(args.name.as_deref(), Some("cd36"));
        assert_eq!(args.country.as_deref(), Some("be"));
    }

    #[test]
    fn prefixed_name_prefixed_rank() {
        let args = Args::new("user=cd36 rank=123", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert_eq!(args.name.as_deref(), Some("cd36"));
        assert!(args.country.is_none());
    }

    #[test]
    fn prefixed_name_prefixed_country_rank() {
        let args = Args::new("user=cd36 rank=be123", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert_eq!(args.name.as_deref(), Some("cd36"));
        assert_eq!(args.country.as_deref(), Some("be"));
    }

    #[test]
    fn two_ranks() {
        let args = Args::new("cd36 be123", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "36");
        assert_eq!(args.name.as_deref(), Some("be123"));
        assert_eq!(args.country.as_deref(), Some("cd"));
    }

    #[test]
    fn two_ranks_first_prefixed() {
        let args = Args::new("rank=cd36 be123", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "36");
        assert_eq!(args.name.as_deref(), Some("be123"));
        assert_eq!(args.country.as_deref(), Some("cd"));
    }

    #[test]
    fn two_ranks_second_prefixed() {
        let args = Args::new("cd36 rank=be123", ArgsNum::None);
        let args = RankPp::args(None, args).unwrap();

        assert_eq!(args.rank, "123");
        assert_eq!(args.name.as_deref(), Some("cd36"));
        assert_eq!(args.country.as_deref(), Some("be"));
    }
}
