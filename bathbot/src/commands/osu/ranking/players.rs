use std::{mem, ops::Deref, sync::Arc};

use bathbot_macros::command;
use bathbot_model::{CountryCode, RankingEntries, RankingEntry, RankingKind};
use bathbot_util::constants::{GENERAL_ISSUE, OSU_API_ISSUE};
use eyre::{Report, Result};
use rosu_v2::prelude::{GameMode, OsuResult, Rankings};

use crate::{
    commands::GameModeOption,
    core::commands::CommandOrigin,
    manager::redis::{osu::UserArgs, RedisData},
    pagination::RankingPagination,
    util::ChannelExt,
    Context,
};

use super::{RankingPp, RankingScore};

// TODO: this sucks
fn check_country(arg: &str) -> Result<CountryCode, &'static str> {
    if arg.len() == 2 && arg.is_ascii() {
        Ok(arg.into())
    } else if let Some(code) = CountryCode::from_name(arg) {
        Ok(code)
    } else {
        Err("The given argument must be a valid country or country code of two ASCII letters")
    }
}

pub(super) async fn pp(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: RankingPp<'_>,
) -> Result<()> {
    let RankingPp { country, mode } = args;
    let author_id = orig.user_id()?;

    let (mode, author_id) = match mode {
        Some(mode) => match ctx.user_config().osu_id(author_id).await {
            Ok(user_id) => (mode.into(), user_id),
            Err(err) => {
                warn!("{:?}", err.wrap_err("failed to get author id"));

                (mode.into(), None)
            }
        },
        None => match ctx.user_config().with_osu_id(author_id).await {
            Ok(config) => (config.mode.unwrap_or(GameMode::Osu), config.osu),
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err.wrap_err("failed to get user config"));
            }
        },
    };

    let country = match country.as_deref() {
        Some(country) => {
            if country.len() != 2 {
                match CountryCode::from_name(country) {
                    Some(code) => Some(code),
                    None => {
                        let content = format!(
                            "Looks like `{country}` is neither a country name nor a country code"
                        );

                        return orig.error(&ctx, content).await;
                    }
                }
            } else {
                Some(country.to_uppercase().into())
            }
        }
        None => None,
    };

    let ranking_fut = async {
        let ranking_result = match country.as_deref() {
            Some(country) => ctx.redis().pp_ranking(mode, 1, Some(country)).await,
            None => ctx.redis().pp_ranking(mode, 1, None).await,
        };

        ranking_result.map(|ranking| match ranking {
            RedisData::Original(ranking) => ranking,
            RedisData::Archived(ranking) => ranking.deserialize(),
        })
    };

    let author_idx_fut = pp_author_idx(&ctx, author_id, mode, country.as_ref());

    let (ranking_res, author_idx) = tokio::join!(ranking_fut, author_idx_fut);
    let kind = OsuRankingKind::Performance;

    ranking(ctx, orig, mode, country, kind, author_idx, ranking_res).await
}

async fn pp_author_idx(
    ctx: &Context,
    author_id: Option<u32>,
    mode: GameMode,
    country: Option<&CountryCode>,
) -> Option<usize> {
    let user_args = UserArgs::user_id(author_id?).mode(mode);

    match ctx.redis().osu_user(user_args).await {
        Ok(user) => {
            let idx = match country {
                Some(code) => {
                    if user.country_code() == code.as_str() {
                        user.peek_stats(|stats| stats.country_rank)
                    } else {
                        None
                    }
                }
                None => user.peek_stats(|stats| stats.global_rank),
            };

            idx.filter(|n| (1..=10_000).contains(n))
                .map(|n| n as usize - 1)
        }
        Err(err) => {
            let report = Report::new(err).wrap_err("Failed to get user");
            warn!("{report:?}");

            None
        }
    }
}

pub(super) async fn score(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: RankingScore,
) -> Result<()> {
    let author_id = orig.user_id()?;

    let (mode, osu_id) = match args.mode.map(GameMode::from) {
        Some(mode) => match ctx.user_config().osu_id(author_id).await {
            Ok(user_id) => (mode, user_id),
            Err(err) => {
                warn!("{err:?}");

                (mode, None)
            }
        },
        None => match ctx.user_config().with_osu_id(author_id).await {
            Ok(config) => (config.mode.unwrap_or(GameMode::Osu), config.osu),
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err.wrap_err("failed to get user config"));
            }
        },
    };

    let ranking_fut = ctx.osu().score_rankings(mode);

    let author_idx_fut = async {
        match osu_id {
            Some(user_id) => match ctx.client().get_respektive_user(user_id, mode).await {
                Ok(Some(user)) => Some(user.rank as usize - 1),
                Ok(None) => None,
                Err(err) => {
                    warn!("{:?}", err.wrap_err("failed to get respektive user"));

                    None
                }
            },
            None => None,
        }
    };

    let (ranking_result, author_idx) = tokio::join!(ranking_fut, author_idx_fut);
    let kind = OsuRankingKind::Score;

    ranking(ctx, orig, mode, None, kind, author_idx, ranking_result).await
}

async fn ranking(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    mode: GameMode,
    country: Option<CountryCode>,
    kind: OsuRankingKind,
    author_idx: Option<usize>,
    result: OsuResult<Rankings>,
) -> Result<()> {
    let mut ranking = match result {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(Report::new(err).wrap_err("failed to get ranking"));
        }
    };

    let country = country.map(|code| {
        let name = ranking
            .ranking
            .get_mut(0)
            .and_then(|user| mem::take(&mut user.country))
            .unwrap_or_else(|| code.as_str().to_owned());

        (name, code)
    });

    let total = ranking.total as usize;

    let entries = match kind {
        OsuRankingKind::Performance => {
            let entries = ranking
                .ranking
                .into_iter()
                .map(|user| RankingEntry {
                    country: Some(user.country_code.into()),
                    name: user.username,
                    value: user.statistics.as_ref().expect("missing stats").pp.round() as u32,
                })
                .enumerate()
                .collect();

            RankingEntries::PpU32(entries)
        }
        OsuRankingKind::Score => {
            let entries = ranking
                .ranking
                .into_iter()
                .map(|user| RankingEntry {
                    country: Some(user.country_code.into()),
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

    let builder = RankingPagination::builder(entries, total, author_idx, ranking_kind);

    builder
        .start_by_update()
        .defer_components()
        .start(ctx, orig)
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
pub async fn prefix_ppranking(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> Result<()> {
    let country = match args.next().map(check_country) {
        Some(Ok(arg)) => Some(arg),
        Some(Err(content)) => {
            msg.error(&ctx, content).await?;

            return Ok(());
        }
        None => None,
    };

    let args = RankingPp {
        mode: None,
        country: country.map(|c| c.deref().clone().into_string().into()),
    };

    pp(ctx, msg.into(), args).await
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
pub async fn prefix_pprankingmania(
    ctx: Arc<Context>,
    msg: &Message,
    mut args: Args<'_>,
) -> Result<()> {
    let country = match args.next().map(check_country) {
        Some(Ok(arg)) => Some(arg),
        Some(Err(content)) => {
            msg.error(&ctx, content).await?;

            return Ok(());
        }
        None => None,
    };

    let args = RankingPp {
        mode: Some(GameModeOption::Mania),
        country: country.map(|c| c.deref().clone().into_string().into()),
    };

    pp(ctx, msg.into(), args).await
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
pub async fn prefix_pprankingtaiko(
    ctx: Arc<Context>,
    msg: &Message,
    mut args: Args<'_>,
) -> Result<()> {
    let country = match args.next().map(check_country) {
        Some(Ok(arg)) => Some(arg),
        Some(Err(content)) => {
            msg.error(&ctx, content).await?;

            return Ok(());
        }
        None => None,
    };

    let args = RankingPp {
        mode: Some(GameModeOption::Taiko),
        country: country.map(|c| c.deref().clone().into_string().into()),
    };

    pp(ctx, msg.into(), args).await
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
pub async fn prefix_pprankingctb(
    ctx: Arc<Context>,
    msg: &Message,
    mut args: Args<'_>,
) -> Result<()> {
    let country = match args.next().map(check_country) {
        Some(Ok(arg)) => Some(arg),
        Some(Err(content)) => {
            msg.error(&ctx, content).await?;

            return Ok(());
        }
        None => None,
    };

    let args = RankingPp {
        mode: Some(GameModeOption::Catch),
        country: country.map(|c| c.deref().clone().into_string().into()),
    };

    pp(ctx, msg.into(), args).await
}

#[command]
#[desc("Display the global osu! ranked score ranking")]
#[aliases("rsr", "rslb")]
#[group(Osu)]
pub async fn prefix_rankedscoreranking(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    score(ctx, msg.into(), None.into()).await
}

#[command]
#[desc("Display the global osu!mania ranked score ranking")]
#[aliases("rsrm", "rslbm")]
#[group(Mania)]
pub async fn prefix_rankedscorerankingmania(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    score(ctx, msg.into(), Some(GameModeOption::Mania).into()).await
}

#[command]
#[desc("Display the global osu!taiko ranked score ranking")]
#[aliases("rsrt", "rslbt")]
#[group(Taiko)]
pub async fn prefix_rankedscorerankingtaiko(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    score(ctx, msg.into(), Some(GameModeOption::Taiko).into()).await
}

#[command]
#[desc("Display the global osu!ctb ranked score ranking")]
#[aliases("rsrc", "rslbc")]
#[group(Catch)]
pub async fn prefix_rankedscorerankingctb(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    score(ctx, msg.into(), Some(GameModeOption::Catch).into()).await
}

#[derive(Eq, PartialEq)]
pub enum OsuRankingKind {
    Performance,
    Score,
}
