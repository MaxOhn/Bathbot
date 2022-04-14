use std::{borrow::Cow, collections::BTreeMap, fmt, mem, sync::Arc};

use chrono::{DateTime, Utc};
use command_macros::command;
use eyre::Report;
use lazy_static::__Deref;
use rosu_v2::prelude::{GameMode, OsuResult, Rankings};

use crate::{
    commands::GameModeOption,
    core::commands::CommandOrigin,
    embeds::{EmbedData, RankingEmbed, RankingEntry, RankingKindData},
    pagination::{Pagination, RankingPagination},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        numbers, ChannelExt, CountryCode,
    },
    BotResult, Context,
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
) -> BotResult<()> {
    let RankingPp { country, mode } = args;

    let mode = match mode {
        Some(mode) => mode.into(),
        None => match ctx.user_config(orig.user_id()?).await {
            Ok(config) => config.mode.unwrap_or(GameMode::STD),
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let result = match country.as_deref() {
        Some(country) => {
            let country = if country.len() != 2 {
                match CountryCode::from_name(country) {
                    Some(code) => code,
                    None => {
                        let content = format!(
                            "Looks like `{country}` is neither a country name nor a country code"
                        );

                        return orig.error(&ctx, content).await;
                    }
                }
            } else {
                country.into()
            };

            ctx.osu()
                .performance_rankings(mode)
                .country(country.as_str())
                .await
        }
        None => ctx.osu().performance_rankings(mode).await,
    };

    let kind = RankingKind::Performance;

    ranking(ctx, orig, mode, country, kind, result).await
}

pub(super) async fn score(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: RankingScore,
) -> BotResult<()> {
    let mode = match args.mode {
        Some(mode) => mode.into(),
        None => match ctx.user_config(orig.user_id()?).await {
            Ok(config) => config.mode.unwrap_or(GameMode::STD),
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let result = ctx.osu().score_rankings(mode).await;

    ranking(ctx, orig, mode, None, RankingKind::Score, result).await
}

async fn ranking(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    mode: GameMode,
    country_code: Option<Cow<'_, str>>,
    kind: RankingKind,
    result: OsuResult<Rankings>,
) -> BotResult<()> {
    let mut ranking = match result {
        Ok(ranking) => ranking,
        Err(why) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let country = match country_code {
        Some(country) => match CountryCode::from_name(&country) {
            Some(code) => Some(code),
            None => {
                if country.len() == 2 {
                    Some(country.into())
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

    let country = country.map(|code| {
        let name = ranking
            .ranking
            .get_mut(0)
            .and_then(|user| mem::take(&mut user.country))
            .unwrap_or_else(|| code.as_str().to_owned());

        (name, code)
    });

    let total = ranking.total as usize;
    let pages = numbers::div_euclid(20, total);

    let users: BTreeMap<_, _> = ranking
        .ranking
        .into_iter()
        .map(|user| {
            let stats = user.statistics.as_ref().unwrap();

            let value = match kind {
                RankingKind::Performance => UserValue::PpU32(stats.pp.round() as u32),
                RankingKind::Score => UserValue::Amount(stats.ranked_score),
            };

            RankingEntry {
                value,
                name: user.username,
                country: user.country_code.into(),
            }
        })
        .enumerate()
        .collect();

    let ranking_kind_data = if let Some((name, code)) = country {
        RankingKindData::PpCountry {
            mode,
            country_code: code.into(),
            country: name,
        }
    } else if kind == RankingKind::Performance {
        RankingKindData::PpGlobal { mode }
    } else {
        RankingKindData::RankedScore { mode }
    };

    let embed_data = RankingEmbed::new(&users, &ranking_kind_data, None, (1, pages));

    // Creating the embed
    let builder = embed_data.into_builder().build().into();
    let response = orig.create_message(&ctx, &builder).await?.model().await?;

    // Pagination
    let pagination = RankingPagination::new(
        response,
        Arc::clone(&ctx),
        total,
        users,
        None,
        ranking_kind_data,
    );

    let owner = orig.user_id()?;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
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
pub async fn prefix_ppranking(
    ctx: Arc<Context>,
    msg: &Message,
    mut args: Args<'_>,
) -> BotResult<()> {
    let country = match args.next().map(check_country) {
        Some(Ok(arg)) => Some(arg),
        Some(Err(content)) => {
            msg.error(&ctx, content).await?;

            return Ok(());
        }
        None => None,
    };

    let args = RankingPp {
        mode: Some(GameModeOption::Osu),
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
) -> BotResult<()> {
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
) -> BotResult<()> {
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
#[aliases("pprc", "pplbc", "ppleaderboardctb")]
#[group(Catch)]
pub async fn prefix_pprankingctb(
    ctx: Arc<Context>,
    msg: &Message,
    mut args: Args<'_>,
) -> BotResult<()> {
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
pub async fn prefix_rankedscoreranking(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    score(ctx, msg.into(), GameModeOption::Osu.into()).await
}

#[command]
#[desc("Display the global osu!mania ranked score ranking")]
#[aliases("rsrm", "rslbm")]
#[group(Mania)]
pub async fn prefix_rankedscorerankingmania(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    score(ctx, msg.into(), GameModeOption::Mania.into()).await
}

#[command]
#[desc("Display the global osu!taiko ranked score ranking")]
#[aliases("rsrt", "rslbt")]
#[group(Taiko)]
pub async fn prefix_rankedscorerankingtaiko(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    score(ctx, msg.into(), GameModeOption::Taiko.into()).await
}

#[command]
#[desc("Display the global osu!ctb ranked score ranking")]
#[aliases("rsrc", "rslbc")]
#[group(Catch)]
pub async fn prefix_rankedscorerankingctb(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    score(ctx, msg.into(), GameModeOption::Catch.into()).await
}

#[derive(Eq, PartialEq)]
pub enum RankingKind {
    Performance,
    Score,
}

#[derive(Copy, Clone)]
pub enum UserValue {
    Accuracy(f32),
    Amount(u64),
    AmountWithNegative(i64),
    Date(DateTime<Utc>),
    Float(f32),
    Playtime(u32),
    PpF32(f32),
    PpU32(u32),
    Rank(u32),
}

impl fmt::Display for UserValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Accuracy(acc) => write!(f, "{:.2}%", numbers::round(acc)),
            Self::Amount(amount) => write!(f, "{}", Self::AmountWithNegative(amount as i64)),
            Self::AmountWithNegative(amount) => {
                if amount.abs() < 1_000_000_000 {
                    write!(f, "{}", numbers::with_comma_int(amount))
                } else {
                    let score = (amount / 10_000_000) as f32 / 100.0;

                    write!(f, "{score:.2} bn")
                }
            }
            Self::Date(date) => write!(f, "{}", date.format("%F")),
            Self::Float(v) => write!(f, "{:.2}", numbers::round(v)),
            Self::Playtime(seconds) => {
                write!(f, "{} hrs", numbers::with_comma_int(seconds / 60 / 60))
            }
            Self::PpF32(pp) => write!(f, "{}pp", numbers::with_comma_float(numbers::round(pp))),
            Self::PpU32(pp) => write!(f, "{}pp", numbers::with_comma_int(pp)),
            Self::Rank(rank) => write!(f, "#{}", numbers::with_comma_int(rank)),
        }
    }
}
