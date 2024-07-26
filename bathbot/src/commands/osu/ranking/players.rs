use std::{borrow::Cow, iter, mem};

use bathbot_macros::command;
use bathbot_model::{
    rosu_v2::ranking::Rankings, Countries, Either, RankingEntries, RankingEntry, RankingKind,
};
use bathbot_util::constants::{GENERAL_ISSUE, OSU_API_ISSUE};
use eyre::{Report, Result};
use rosu_v2::prelude::{CountryCode, GameMode, OsuResult, Rankings as RosuRankings};

use super::{RankingPp, RankingScore};
use crate::{
    active::{impls::RankingPagination, ActiveMessages},
    commands::GameModeOption,
    core::commands::CommandOrigin,
    manager::redis::{osu::UserArgs, RedisData},
    util::ChannelExt,
    Context,
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

    let (mode, author_id) = match mode {
        Some(mode) => match Context::user_config().osu_id(owner).await {
            Ok(user_id) => (mode.into(), user_id),
            Err(err) => {
                warn!(?err, "Failed to get author id");

                (mode.into(), None)
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

    let ranking_fut = async {
        let country = country.as_deref();

        Context::redis()
            .pp_ranking(mode, 1, country)
            .await
            .map(|ranking| match ranking {
                RedisData::Original(ranking) => Either::Left(ranking),
                RedisData::Archive(ranking) => Either::Right(ranking.deserialize()),
            })
    };

    let author_idx_fut = pp_author_idx(author_id, mode, country.as_ref());

    let (ranking_res, author_idx) = tokio::join!(ranking_fut, author_idx_fut);
    let kind = OsuRankingKind::Performance;

    ranking(orig, mode, country, kind, author_idx, ranking_res).await
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
                    if user.country_code() == code.as_str() {
                        Some(user.stats().country_rank())
                    } else {
                        None
                    }
                }
                None => Some(user.stats().global_rank()),
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

pub(super) async fn score(orig: CommandOrigin<'_>, args: RankingScore) -> Result<()> {
    let owner = orig.user_id()?;

    let (mode, osu_id) = match args.mode.map(GameMode::from) {
        Some(mode) => match Context::user_config().osu_id(owner).await {
            Ok(user_id) => (mode, user_id),
            Err(err) => {
                warn!("{err:?}");

                (mode, None)
            }
        },
        None => match Context::user_config().with_osu_id(owner).await {
            Ok(config) => (config.mode.unwrap_or(GameMode::Osu), config.osu),
            Err(err) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(err.wrap_err("failed to get user config"));
            }
        },
    };

    let ranking_fut = Context::osu().score_rankings(mode);

    let author_idx_fut = async {
        match osu_id.map(iter::once) {
            Some(user_id) => match Context::client().get_respektive_users(user_id, mode).await {
                Ok(mut iter) => iter
                    .next()
                    .flatten()
                    .and_then(|user| user.rank)
                    .map(|rank| rank.get() as usize - 1),
                Err(err) => {
                    warn!(?err, "Failed to get respektive user");

                    None
                }
            },
            None => None,
        }
    };

    let (ranking_res, author_idx) = tokio::join!(ranking_fut, author_idx_fut);
    let ranking_res = ranking_res.map(Either::Left);
    let kind = OsuRankingKind::Score;

    ranking(orig, mode, None, kind, author_idx, ranking_res).await
}

async fn ranking(
    orig: CommandOrigin<'_>,
    mode: GameMode,
    country: Option<CountryCode>,
    kind: OsuRankingKind,
    author_idx: Option<usize>,
    result: OsuResult<Either<RosuRankings, Rankings>>,
) -> Result<()> {
    let mut ranking = match result {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = orig.error(OSU_API_ISSUE).await;

            return Err(Report::new(err).wrap_err("failed to get ranking"));
        }
    };

    let country = country.map(|code| {
        let name = match ranking {
            Either::Left(ref mut ranking) => ranking
                .ranking
                .get_mut(0)
                .and_then(|user| mem::take(&mut user.country))
                .map(String::into_boxed_str),
            Either::Right(ref mut ranking) => ranking
                .ranking
                .get_mut(0)
                .and_then(|user| mem::take(&mut user.country)),
        };

        (name.unwrap_or_else(|| Box::from(code.as_str())), code)
    });

    let total = match ranking {
        Either::Left(ref ranking) => ranking.total as usize,
        Either::Right(ref ranking) => ranking.total as usize,
    };

    let entries = match kind {
        OsuRankingKind::Performance => {
            let entries = match ranking {
                Either::Left(ranking) => ranking
                    .ranking
                    .into_iter()
                    .map(|user| RankingEntry {
                        country: Some(user.country_code),
                        name: user.username,
                        value: user.statistics.as_ref().expect("missing stats").pp.round() as u32,
                    })
                    .enumerate()
                    .collect(),
                Either::Right(ranking) => ranking
                    .ranking
                    .into_iter()
                    .map(|user| RankingEntry {
                        country: Some(user.country_code),
                        name: user.username,
                        value: user.statistics.as_ref().expect("missing stats").pp.round() as u32,
                    })
                    .enumerate()
                    .collect(),
            };

            RankingEntries::PpU32(entries)
        }
        OsuRankingKind::Score => {
            let entries = match ranking {
                Either::Left(ranking) => ranking
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
                    .collect(),
                Either::Right(ranking) => ranking
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
                    .collect(),
            };

            RankingEntries::Amount(entries)
        }
    };

    let ranking_kind = if let Some((name, code)) = country {
        RankingKind::PpCountry {
            mode,
            country_code: code,
            country: name,
        }
    } else if kind == OsuRankingKind::Performance {
        RankingKind::PpGlobal { mode }
    } else {
        RankingKind::RankedScore { mode }
    };

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

#[derive(Eq, PartialEq)]
pub enum OsuRankingKind {
    Performance,
    Score,
}
