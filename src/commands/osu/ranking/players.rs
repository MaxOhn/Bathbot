use crate::{
    embeds::{EmbedData, RankingEmbed, RankingEntry, RankingKindData},
    pagination::{Pagination, RankingPagination},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        numbers, CountryCode, CowUtils, MessageExt,
    },
    BotResult, CommandData, Context,
};

use chrono::{DateTime, Utc};
use eyre::Report;
use rosu_v2::prelude::{GameMode, OsuResult, Rankings};
use std::{collections::BTreeMap, fmt, mem, sync::Arc};

fn country_code_(arg: &str) -> Result<CountryCode, &'static str> {
    if arg.len() == 2 && arg.is_ascii() {
        Ok(arg.to_ascii_uppercase().into())
    } else if let Some(code) = CountryCode::from_name(arg) {
        Ok(code)
    } else {
        Err("The given argument must be a valid country or country code of two ASCII letters")
    }
}

pub(super) async fn _performanceranking(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    mut mode: GameMode,
    country_code: Option<CountryCode>,
) -> BotResult<()> {
    if mode == GameMode::STD {
        mode = match ctx.user_config(data.author()?.id).await {
            Ok(config) => config.mode.unwrap_or(GameMode::STD),
            Err(why) => {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }
        };
    }

    let result = match country_code {
        Some(ref country) => {
            ctx.osu()
                .performance_rankings(mode)
                .country(country.as_str())
                .await
        }
        None => ctx.osu().performance_rankings(mode).await,
    };

    let kind = RankingKind::Performance;

    _ranking(ctx, data, mode, country_code, kind, result).await
}

pub(super) async fn _scoreranking(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    mut mode: GameMode,
) -> BotResult<()> {
    if mode == GameMode::STD {
        mode = match ctx.user_config(data.author()?.id).await {
            Ok(config) => config.mode.unwrap_or(GameMode::STD),
            Err(why) => {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }
        };
    }

    let result = ctx.osu().score_rankings(mode).await;

    _ranking(ctx, data, mode, None, RankingKind::Score, result).await
}

async fn _ranking(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    mode: GameMode,
    country_code: Option<CountryCode>,
    kind: RankingKind,
    result: OsuResult<Rankings>,
) -> BotResult<()> {
    let mut ranking = match result {
        Ok(ranking) => ranking,
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let country = country_code.map(|code| {
        let name = ranking
            .ranking
            .get_mut(0)
            .and_then(|user| mem::take(&mut user.country))
            .unwrap_or_else(|| code.to_string());

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
            country_code: code,
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
    let response = data.create_message(&ctx, builder).await?.model().await?;

    // Pagination
    let pagination = RankingPagination::new(
        response,
        Arc::clone(&ctx),
        total,
        users,
        None,
        ranking_kind_data,
    );

    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display the osu! pp ranking")]
#[long_desc(
    "Display the osu! pp ranking.\n\
    For the global ranking, don't give any arguments.\n\
    For a country specific ranking, provide its name or country code as first argument."
)]
#[usage("[country]")]
#[example("", "de", "russia")]
#[aliases("ppr", "pplb", "ppleaderboard")]
pub async fn ppranking(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let arg = args.next().map(CowUtils::cow_to_ascii_lowercase);

            let country = match arg.as_deref().map(country_code_).transpose() {
                Ok(country) => country,
                Err(content) => return msg.error(&ctx, content).await,
            };

            let data = CommandData::Message { msg, args, num };

            _performanceranking(ctx, data, GameMode::STD, country).await
        }
        CommandData::Interaction { command } => super::slash_ranking(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display the osu!mania pp ranking")]
#[long_desc(
    "Display the osu!mania pp ranking.\n\
    For the global ranking, don't give any arguments.\n\
    For a country specific ranking, provide its name or country code as first argument."
)]
#[usage("[country]")]
#[example("", "de", "russia")]
#[aliases("pprm", "pplbm", "ppleaderboardmania")]
pub async fn pprankingmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let country = match args.next().map(country_code_).transpose() {
                Ok(country) => country,
                Err(content) => return msg.error(&ctx, content).await,
            };

            let data = CommandData::Message { msg, args, num };

            _performanceranking(ctx, data, GameMode::MNA, country).await
        }
        CommandData::Interaction { command } => super::slash_ranking(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display the osu!taiko pp ranking")]
#[long_desc(
    "Display the osu!taiko pp ranking.\n\
    For the global ranking, don't give any arguments.\n\
    For a country specific ranking, provide its name or country code as first argument."
)]
#[usage("[country]")]
#[example("", "de", "russia")]
#[aliases("pprt", "pplbt", "ppleaderboardtaiko")]
pub async fn pprankingtaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let country = match args.next().map(country_code_).transpose() {
                Ok(country) => country,
                Err(content) => return msg.error(&ctx, content).await,
            };

            let data = CommandData::Message { msg, args, num };

            _performanceranking(ctx, data, GameMode::TKO, country).await
        }
        CommandData::Interaction { command } => super::slash_ranking(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display the osu!ctb pp ranking")]
#[long_desc(
    "Display the osu!ctb pp ranking.\n\
    For the global ranking, don't give any arguments.\n\
    For a country specific ranking, provide its name or country code as first argument."
)]
#[usage("[country]")]
#[example("", "de", "russia")]
#[aliases("pprc", "pplbc", "ppleaderboardctb")]
pub async fn pprankingctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let country = match args.next().map(country_code_).transpose() {
                Ok(country) => country,
                Err(content) => return msg.error(&ctx, content).await,
            };

            let data = CommandData::Message { msg, args, num };

            _performanceranking(ctx, data, GameMode::CTB, country).await
        }
        CommandData::Interaction { command } => super::slash_ranking(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display the global osu! ranked score ranking")]
#[aliases("rsr", "rslb")]
pub async fn rankedscoreranking(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        data @ CommandData::Message { .. } => _scoreranking(ctx, data, GameMode::STD).await,
        CommandData::Interaction { command } => super::slash_ranking(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display the global osu!mania ranked score ranking")]
#[aliases("rsrm", "rslbm")]
pub async fn rankedscorerankingmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        data @ CommandData::Message { .. } => _scoreranking(ctx, data, GameMode::MNA).await,
        CommandData::Interaction { command } => super::slash_ranking(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display the global osu!taiko ranked score ranking")]
#[aliases("rsrt", "rslbt")]
pub async fn rankedscorerankingtaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        data @ CommandData::Message { .. } => _scoreranking(ctx, data, GameMode::TKO).await,
        CommandData::Interaction { command } => super::slash_ranking(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display the global osu!ctb ranked score ranking")]
#[aliases("rsrc", "rslbc")]
pub async fn rankedscorerankingctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        data @ CommandData::Message { .. } => _scoreranking(ctx, data, GameMode::CTB).await,
        CommandData::Interaction { command } => super::slash_ranking(ctx, *command).await,
    }
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
    Level(f32),
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
            Self::Level(level) => write!(f, "{:.2}", numbers::round(level)),
            Self::Playtime(seconds) => {
                write!(f, "{} hrs", numbers::with_comma_int(seconds / 60 / 60))
            }
            Self::PpF32(pp) => write!(f, "{}pp", numbers::with_comma_float(numbers::round(pp))),
            Self::PpU32(pp) => write!(f, "{}pp", numbers::with_comma_int(pp)),
            Self::Rank(rank) => write!(f, "#{}", numbers::with_comma_int(rank)),
        }
    }
}
