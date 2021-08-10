use crate::{
    embeds::{EmbedData, RankingEmbed},
    pagination::{Pagination, RankingPagination},
    util::{constants::OSU_API_ISSUE, numbers, MessageExt},
    BotResult, CommandData, Context,
};

use rosu_v2::prelude::{GameMode, OsuResult, Rankings};
use std::{borrow::Cow, collections::BTreeMap, fmt, sync::Arc};

fn check_country_code(mut code: String) -> Result<String, &'static str> {
    if code.len() == 2 && code.chars().all(|c| c.is_ascii_alphabetic()) {
        code.make_ascii_uppercase();

        Ok(code)
    } else {
        Err("The given argument must be a country code of two ASCII letters")
    }
}

pub(super) async fn _performanceranking(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    mode: GameMode,
    country: Option<String>,
) -> BotResult<()> {
    let country_code = match country.map(check_country_code).transpose() {
        Ok(country) => country,
        Err(content) => return data.error(&ctx, content).await,
    };

    let result = match country_code.as_ref() {
        Some(country) => ctx.osu().performance_rankings(mode).country(country).await,
        None => ctx.osu().performance_rankings(mode).await,
    };

    let kind = RankingKind::Performance;

    _ranking(ctx, data, mode, country_code, kind, result).await
}

pub(super) async fn _scoreranking(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    mode: GameMode,
) -> BotResult<()> {
    let result = ctx.osu().score_rankings(mode).await;

    _ranking(ctx, data, mode, None, RankingKind::Score, result).await
}

async fn _ranking(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    mode: GameMode,
    country_code: Option<String>,
    kind: RankingKind,
    result: OsuResult<Rankings>,
) -> BotResult<()> {
    let ranking = match result {
        Ok(ranking) => ranking,
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let country_name = country_code.as_ref().map(|_| {
        ranking
            .ranking
            .get(0)
            .and_then(|user| user.country.as_deref())
            .unwrap_or("XX")
    });

    let url_type = kind.url_type();
    let title = kind.title(country_name);

    let total = ranking.total as usize;
    let pages = numbers::div_euclid(20, total);

    let users: BTreeMap<_, _> = ranking
        .ranking
        .into_iter()
        .map(|user| {
            let key = UserValue::Pp(user.statistics.as_ref().unwrap().pp.round() as u32);

            (key, user.username)
        })
        .enumerate()
        .collect();

    let embed_data = RankingEmbed::new(
        mode,
        &users,
        &title,
        url_type,
        country_code.as_deref(),
        (1, pages),
    );

    // Creating the embed
    let builder = embed_data.into_builder().build().into();
    let response_raw = data.create_message(&ctx, builder).await?;
    let response = data.get_response(&ctx, response_raw).await?;

    // Pagination
    let pagination = RankingPagination::new(
        response,
        mode,
        Arc::clone(&ctx),
        total,
        users,
        title,
        url_type,
        country_code,
        RankingKind::Performance,
    );

    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (ranking): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display the osu! pp ranking")]
#[long_desc(
    "Display the osu! pp ranking.\n\
    For the global ranking, don't give any arguments.\n\
    For a country specific ranking, provide its country code as first argument."
)]
#[usage("[country code]")]
#[example("", "de")]
#[aliases("ppr", "pplb", "ppleaderboard")]
pub async fn ppranking(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let country = args.next().map(str::to_owned);
            let data = CommandData::Message { msg, args, num };

            _performanceranking(ctx, data, GameMode::STD, country).await
        }
        CommandData::Interaction { command } => super::slash_ranking(ctx, command).await,
    }
}

#[command]
#[short_desc("Display the osu!mania pp ranking")]
#[long_desc(
    "Display the osu!mania pp ranking.\n\
    For the global ranking, don't give any arguments.\n\
    For a country specific ranking, provide its country code as first argument."
)]
#[usage("[country code]")]
#[example("", "de")]
#[aliases("pprm", "pplbm", "ppleaderboardmania")]
pub async fn pprankingmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let country = args.next().map(str::to_owned);
            let data = CommandData::Message { msg, args, num };

            _performanceranking(ctx, data, GameMode::MNA, country).await
        }
        CommandData::Interaction { command } => super::slash_ranking(ctx, command).await,
    }
}

#[command]
#[short_desc("Display the osu!taiko pp ranking")]
#[long_desc(
    "Display the osu!taiko pp ranking.\n\
    For the global ranking, don't give any arguments.\n\
    For a country specific ranking, provide its country code as first argument."
)]
#[usage("[country code]")]
#[example("", "de")]
#[aliases("pprt", "pplbt", "ppleaderboardtaiko")]
pub async fn pprankingtaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let country = args.next().map(str::to_owned);
            let data = CommandData::Message { msg, args, num };

            _performanceranking(ctx, data, GameMode::TKO, country).await
        }
        CommandData::Interaction { command } => super::slash_ranking(ctx, command).await,
    }
}

#[command]
#[short_desc("Display the osu!ctb pp ranking")]
#[long_desc(
    "Display the osu!ctb pp ranking.\n\
    For the global ranking, don't give any arguments.\n\
    For a country specific ranking, provide its country code as first argument."
)]
#[usage("[country code]")]
#[example("", "de")]
#[aliases("pprc", "pplbc", "ppleaderboardctb")]
pub async fn pprankingctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let country = args.next().map(str::to_owned);
            let data = CommandData::Message { msg, args, num };

            _performanceranking(ctx, data, GameMode::CTB, country).await
        }
        CommandData::Interaction { command } => super::slash_ranking(ctx, command).await,
    }
}

#[command]
#[short_desc("Display the global osu! ranked score ranking")]
#[aliases("rsr", "rslb")]
pub async fn rankedscoreranking(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        data @ CommandData::Message { .. } => _scoreranking(ctx, data, GameMode::STD).await,
        CommandData::Interaction { command } => super::slash_ranking(ctx, command).await,
    }
}

#[command]
#[short_desc("Display the global osu!mania ranked score ranking")]
#[aliases("rsrm", "rslbm")]
pub async fn rankedscorerankingmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        data @ CommandData::Message { .. } => _scoreranking(ctx, data, GameMode::MNA).await,
        CommandData::Interaction { command } => super::slash_ranking(ctx, command).await,
    }
}

#[command]
#[short_desc("Display the global osu!taiko ranked score ranking")]
#[aliases("rsrt", "rslbt")]
pub async fn rankedscorerankingtaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        data @ CommandData::Message { .. } => _scoreranking(ctx, data, GameMode::TKO).await,
        CommandData::Interaction { command } => super::slash_ranking(ctx, command).await,
    }
}

#[command]
#[short_desc("Display the global osu!ctb ranked score ranking")]
#[aliases("rsrc", "rslbc")]
pub async fn rankedscorerankingctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        data @ CommandData::Message { .. } => _scoreranking(ctx, data, GameMode::CTB).await,
        CommandData::Interaction { command } => super::slash_ranking(ctx, command).await,
    }
}

#[derive(Copy, Clone)]
pub enum RankingKind {
    Performance,
    Score,
}

impl RankingKind {
    fn url_type(self) -> &'static str {
        match self {
            RankingKind::Performance => "performance",
            RankingKind::Score => "score",
        }
    }

    fn title(self, country: Option<&str>) -> Cow<'static, str> {
        match (self, country) {
            (RankingKind::Performance, None) => "Performance".into(),
            (RankingKind::Performance, Some(country)) => format!(
                "{name}'{plural} Performance",
                name = country,
                plural = if country.ends_with('s') { "" } else { "s" }
            )
            .into(),
            (RankingKind::Score, _) => "Ranked Score".into(),
        }
    }
}

#[derive(Copy, Clone)]
pub enum UserValue {
    Pp(u32),
    Score(u64),
}

impl fmt::Display for UserValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            UserValue::Pp(pp) => write!(f, "{}pp", numbers::with_comma_uint(pp)),
            UserValue::Score(score) => {
                if score < 1_000_000 {
                    write!(f, "{}", score)
                } else if score < 1_000_000_000 {
                    let score = (score / 10_000) as f32 / 100.0;

                    write!(f, "{:.2} million", score)
                } else {
                    let score = (score / 10_000_000) as f32 / 100.0;

                    write!(f, "{:.2} bn", score)
                }
            }
        }
    }
}
