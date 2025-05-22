use std::{borrow::Cow, iter};

use bathbot_cache::model::CachedArchive;
use bathbot_macros::command;
use bathbot_model::{
    Countries, RankingEntries, RankingEntry, RankingKind, command_fields::GameModeOption,
    rosu_v2::ranking::ArchivedRankings,
};
use bathbot_util::constants::GENERAL_ISSUE;
use eyre::{Report, Result};
use rosu_v2::prelude::{CountryCode, GameMode, Rankings};

use super::{RankingPp, RankingScore};
use crate::{
    Context,
    active::{ActiveMessages, impls::RankingPagination},
    core::commands::CommandOrigin,
    manager::redis::{RedisError, osu::UserArgs},
    util::ChannelExt,
};

// TODO: this sucks
fn check_country(arg: &str) -> Result<CountryCode, &'static str> {
    if arg.len() == 2 && arg.is_ascii() {
        Ok(arg.into())
    } else if let Some(code) = Countries::name(arg).to_code() {
        Ok(code.into())
    } else {
        Err("The given argument must be a valid country or country code of two ASCII letters")
    }
}

pub(super) async fn pp(orig: CommandOrigin<'_>, args: RankingPp<'_>) -> Result<()> {
    let RankingPp { country, mode } = args;
    let owner = orig.user_id()?;

    let (mode, author_id) = match mode.map(GameMode::from) {
        Some(mode) => match Context::user_config().osu_id(owner).await {
            Ok(user_id) => (mode, user_id),
            Err(err) => {
                warn!(?err, "Failed to get author id");

                (mode, None)
            }
        },
        None => match Context::user_config().with_osu_id(owner).await {
            Ok(config) => (config.mode.unwrap_or(GameMode::Osu), config.osu),
            Err(err) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(err.wrap_err("Failed to get user config"));
            }
        },
    };

    let country = match country.as_deref() {
        Some(country) if country.len() == 2 => Some(country.to_uppercase().into()),
        Some(country) => match Countries::name(country).to_code() {
            Some(code) => Some(CountryCode::from(code)),
            None => {
                let content =
                    format!("Looks like `{country}` is neither a country name nor a country code");

                return orig.error(content).await;
            }
        },
        None => None,
    };

    let country_ = country.as_deref();
    let ranking_fut = Context::redis().pp_ranking(mode, 1, country_);
    let author_idx_fut = pp_author_idx(author_id, mode, country.as_ref());

    let (ranking_res, author_idx) = tokio::join!(ranking_fut, author_idx_fut);
    let ranking_res = ranking_res.map(Ranking::Performance);

    ranking(orig, mode, country, author_idx, ranking_res).await
}

async fn pp_author_idx(
    author_id: Option<u32>,
    mode: GameMode,
    country: Option<&CountryCode>,
) -> Option<usize> {
    let user_args = UserArgs::user_id(author_id?, mode);

    match Context::redis().osu_user(user_args).await {
        Ok(user) => {
            let idx = match country {
                Some(code) => {
                    if user.country_code.as_str() == code.as_str() {
                        Some(
                            user.statistics
                                .as_ref()
                                .expect("missing stats")
                                .country_rank
                                .to_native(),
                        )
                    } else {
                        None
                    }
                }
                None => Some(
                    user.statistics
                        .as_ref()
                        .expect("missing stats")
                        .global_rank
                        .to_native(),
                ),
            };

            idx.filter(|n| (1..=10_000).contains(n))
                .map(|n| n as usize - 1)
        }
        Err(err) => {
            warn!(?err, "Failed to get user");

            None
        }
    }
}

pub(super) async fn score(orig: CommandOrigin<'_>, args: RankingScore<'_>) -> Result<()> {
    let RankingScore { country, mode } = args;
    let owner = orig.user_id()?;

    let (mode, author_id) = match mode.map(GameMode::from) {
        Some(mode) => match Context::user_config().osu_id(owner).await {
            Ok(user_id) => (mode, user_id),
            Err(err) => {
                warn!(?err, "Failed to get author id");

                (mode, None)
            }
        },
        None => match Context::user_config().with_osu_id(owner).await {
            Ok(config) => (config.mode.unwrap_or(GameMode::Osu), config.osu),
            Err(err) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(err.wrap_err("Failed to get user config"));
            }
        },
    };

    let country = match country.as_deref() {
        Some(country) if country.len() == 2 => Some(country.to_uppercase().into()),
        Some(country) => match Countries::name(country).to_code() {
            Some(code) => Some(CountryCode::from(code)),
            None => {
                let content =
                    format!("Looks like `{country}` is neither a country name nor a country code");

                return orig.error(content).await;
            }
        },
        None => None,
    };

    let mut ranking_fut = Context::osu().score_rankings(mode);

    if let Some(country) = country.as_deref() {
        ranking_fut = ranking_fut.country(country);
    }

    let author_idx_fut = score_author_idx(author_id, mode, country.is_some());

    let (ranking_res, author_idx) = tokio::join!(ranking_fut, author_idx_fut);

    let ranking_res = ranking_res
        .map(Ranking::Score)
        .map_err(Report::new)
        .map_err(RedisError::Acquire);

    ranking(orig, mode, country, author_idx, ranking_res).await
}

async fn score_author_idx(
    author_id: Option<u32>,
    mode: GameMode,
    by_country: bool,
) -> Option<usize> {
    if by_country {
        return None;
    }

    let user_id = author_id.map(iter::once)?;

    let user = match Context::client().get_respektive_users(user_id, mode).await {
        Ok(mut iter) => iter.next().flatten(),
        Err(err) => {
            warn!(?err, "Failed to get respektive user");

            return None;
        }
    };

    user.and_then(|user| user.rank)
        .map(|rank| rank.get() as usize - 1)
}

async fn ranking(
    orig: CommandOrigin<'_>,
    mode: GameMode,
    country: Option<CountryCode>,
    author_idx: Option<usize>,
    result: Result<Ranking, RedisError>,
) -> Result<()> {
    let ranking = match result {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get ranking"));
        }
    };

    let country = country.map(|code| {
        let name = ranking
            .country_name()
            .unwrap_or_else(|| Box::from(code.as_str()));

        (name, code)
    });

    let total = ranking.total();

    let ranking_kind = if let Some((name, code)) = country {
        RankingKind::PpCountry {
            mode,
            country_code: code,
            country: name,
        }
    } else {
        ranking.kind(mode)
    };

    let entries = ranking.entries();

    let pagination = RankingPagination::builder()
        .entries(entries)
        .total(total)
        .author_idx(author_idx)
        .kind(ranking_kind)
        .defer(true)
        .msg_owner(orig.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

enum Ranking {
    Performance(CachedArchive<ArchivedRankings>),
    Score(Rankings),
}

impl Ranking {
    fn country_name(&self) -> Option<Box<str>> {
        let name = match self {
            Ranking::Performance(ranking) => ranking
                .ranking
                .first()
                .and_then(|user| user.country.as_deref()),
            Ranking::Score(ranking) => ranking
                .ranking
                .first()
                .and_then(|user| user.country.as_deref()),
        };

        name.map(Box::from)
    }

    fn total(&self) -> usize {
        match self {
            Ranking::Performance(ranking) => ranking.total.to_native() as usize,
            Ranking::Score(ranking) => ranking.total as usize,
        }
    }

    fn kind(&self, mode: GameMode) -> RankingKind {
        match self {
            Ranking::Performance(_) => RankingKind::PpGlobal { mode },
            Ranking::Score(_) => RankingKind::RankedScore { mode },
        }
    }

    fn entries(self) -> RankingEntries {
        match self {
            Ranking::Performance(ranking) => {
                let entries = ranking
                    .ranking
                    .iter()
                    .map(|user| RankingEntry {
                        country: Some(user.country_code.as_str().into()),
                        name: user.username.as_str().into(),
                        value: user
                            .statistics
                            .as_ref()
                            .expect("missing stats")
                            .pp
                            .to_native()
                            .round() as u32,
                    })
                    .enumerate()
                    .collect();

                RankingEntries::PpU32(entries)
            }
            Ranking::Score(ranking) => {
                let entries = ranking
                    .ranking
                    .into_iter()
                    .map(|user| RankingEntry {
                        country: Some(user.country_code),
                        name: user.username,
                        value: user
                            .statistics
                            .as_ref()
                            .expect("missing stats")
                            .ranked_score,
                    })
                    .enumerate()
                    .collect();

                RankingEntries::Amount(entries)
            }
        }
    }
}

#[command]
#[desc("Display the osu! pp ranking")]
#[help(
    "Display the osu! pp ranking.\n\
    For the global ranking, don't give any arguments.\n\
    For a country specific ranking, provide its name or country code as first argument."
)]
#[usage("[country]")]
#[examples("", "de", "russia")]
#[aliases("ppr", "pplb", "ppleaderboard")]
#[group(Osu)]
pub async fn prefix_ppranking(msg: &Message, mut args: Args<'_>) -> Result<()> {
    let country = match args.next().map(check_country) {
        Some(Ok(arg)) => Some(arg),
        Some(Err(content)) => {
            msg.error(content).await?;

            return Ok(());
        }
        None => None,
    };

    let args = RankingPp {
        mode: None,
        country: country.map(CountryCode::into_string).map(Cow::Owned),
    };

    pp(msg.into(), args).await
}

#[command]
#[desc("Display the osu!mania pp ranking")]
#[help(
    "Display the osu!mania pp ranking.\n\
    For the global ranking, don't give any arguments.\n\
    For a country specific ranking, provide its name or country code as first argument."
)]
#[usage("[country]")]
#[examples("", "de", "russia")]
#[aliases("pprm", "pplbm", "ppleaderboardmania")]
#[group(Mania)]
pub async fn prefix_pprankingmania(msg: &Message, mut args: Args<'_>) -> Result<()> {
    let country = match args.next().map(check_country) {
        Some(Ok(arg)) => Some(arg),
        Some(Err(content)) => {
            msg.error(content).await?;

            return Ok(());
        }
        None => None,
    };

    let args = RankingPp {
        mode: Some(GameModeOption::Mania),
        country: country.map(CountryCode::into_string).map(Cow::Owned),
    };

    pp(msg.into(), args).await
}

#[command]
#[desc("Display the osu!taiko pp ranking")]
#[help(
    "Display the osu!taiko pp ranking.\n\
    For the global ranking, don't give any arguments.\n\
    For a country specific ranking, provide its name or country code as first argument."
)]
#[usage("[country]")]
#[examples("", "de", "russia")]
#[aliases("pprt", "pplbt", "ppleaderboardtaiko")]
#[group(Taiko)]
pub async fn prefix_pprankingtaiko(msg: &Message, mut args: Args<'_>) -> Result<()> {
    let country = match args.next().map(check_country) {
        Some(Ok(arg)) => Some(arg),
        Some(Err(content)) => {
            msg.error(content).await?;

            return Ok(());
        }
        None => None,
    };

    let args = RankingPp {
        mode: Some(GameModeOption::Taiko),
        country: country.map(CountryCode::into_string).map(Cow::Owned),
    };

    pp(msg.into(), args).await
}

#[command]
#[desc("Display the osu!ctb pp ranking")]
#[help(
    "Display the osu!ctb pp ranking.\n\
    For the global ranking, don't give any arguments.\n\
    For a country specific ranking, provide its name or country code as first argument."
)]
#[usage("[country]")]
#[examples("", "de", "russia")]
#[aliases("pprc", "pplbc", "ppleaderboardctb", "pprankingcatch")]
#[group(Catch)]
pub async fn prefix_pprankingctb(msg: &Message, mut args: Args<'_>) -> Result<()> {
    let country = match args.next().map(check_country) {
        Some(Ok(arg)) => Some(arg),
        Some(Err(content)) => {
            msg.error(content).await?;

            return Ok(());
        }
        None => None,
    };

    let args = RankingPp {
        mode: Some(GameModeOption::Catch),
        country: country.map(CountryCode::into_string).map(Cow::Owned),
    };

    pp(msg.into(), args).await
}

#[command]
#[desc("Display the global osu! ranked score ranking")]
#[aliases("rsr", "rslb")]
#[group(Osu)]
pub async fn prefix_rankedscoreranking(msg: &Message) -> Result<()> {
    score(msg.into(), None.into()).await
}

#[command]
#[desc("Display the global osu!mania ranked score ranking")]
#[aliases("rsrm", "rslbm")]
#[group(Mania)]
pub async fn prefix_rankedscorerankingmania(msg: &Message) -> Result<()> {
    score(msg.into(), Some(GameModeOption::Mania).into()).await
}

#[command]
#[desc("Display the global osu!taiko ranked score ranking")]
#[aliases("rsrt", "rslbt")]
#[group(Taiko)]
pub async fn prefix_rankedscorerankingtaiko(msg: &Message) -> Result<()> {
    score(msg.into(), Some(GameModeOption::Taiko).into()).await
}

#[command]
#[desc("Display the global osu!ctb ranked score ranking")]
#[aliases("rsrc", "rslbc")]
#[group(Catch)]
pub async fn prefix_rankedscorerankingctb(msg: &Message) -> Result<()> {
    score(msg.into(), Some(GameModeOption::Catch).into()).await
}
