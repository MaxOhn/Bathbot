use crate::{
    embeds::{EmbedData, RankingEmbed},
    pagination::{Pagination, RankingPagination},
    util::{constants::OSU_API_ISSUE, numbers, MessageExt},
    Args, BotResult, Context,
};

use rosu_v2::prelude::GameMode;
use std::{borrow::Cow, collections::BTreeMap, fmt, sync::Arc};
use twilight_model::channel::Message;

async fn ranking_main(
    mode: GameMode,
    ranking_type: RankingType,
    ctx: Arc<Context>,
    msg: &Message,
    mut args: Args<'_>,
) -> BotResult<()> {
    let country_result = args
        .next()
        .filter(|_| matches!(ranking_type, RankingType::Performance))
        .map(|arg| {
            (arg.len() == 2 && arg.chars().all(|c| c.is_ascii_alphabetic()))
                .then(|| arg.to_ascii_uppercase())
                .ok_or(())
        })
        .transpose();

    let country_code = match country_result {
        Ok(code) => code,
        Err(_) => {
            let content = "The given argument must be a country code of two ASCII letters";

            return msg.error(&ctx, content).await;
        }
    };

    let ranking_result = match (ranking_type, &country_code) {
        (RankingType::Performance, Some(country)) => {
            ctx.osu().performance_rankings(mode).country(country).await
        }
        (RankingType::Performance, None) => ctx.osu().performance_rankings(mode).await,
        (RankingType::Score, _) => ctx.osu().score_rankings(mode).await,
    };

    let ranking = match ranking_result {
        Ok(ranking) => ranking,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

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

    let url_type = ranking_type.url_type();
    let title = ranking_type.title(country_name);

    let total = ranking.total as usize;
    let pages = numbers::div_euclid(20, total);

    let users: BTreeMap<_, _> = ranking
        .ranking
        .into_iter()
        .map(|user| match ranking_type {
            RankingType::Performance => (
                UserValue::Pp(user.statistics.as_ref().unwrap().pp.round() as u32),
                user.username,
            ),
            RankingType::Score => (
                UserValue::Score(user.statistics.as_ref().unwrap().ranked_score),
                user.username,
            ),
        })
        .enumerate()
        .collect();

    let data = RankingEmbed::new(
        mode,
        &users,
        &title,
        url_type,
        country_code.as_deref(),
        (1, pages),
    );

    // Creating the embed
    let response = msg.respond_embed(&ctx, data.into_builder().build()).await?;

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
        ranking_type,
    );

    let owner = msg.author.id;

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
pub async fn ppranking(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    ranking_main(GameMode::STD, RankingType::Performance, ctx, msg, args).await
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
pub async fn pprankingmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    ranking_main(GameMode::MNA, RankingType::Performance, ctx, msg, args).await
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
pub async fn pprankingtaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    ranking_main(GameMode::TKO, RankingType::Performance, ctx, msg, args).await
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
pub async fn pprankingctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    ranking_main(GameMode::CTB, RankingType::Performance, ctx, msg, args).await
}

#[command]
#[short_desc("Display the global osu! ranked score ranking")]
#[aliases("rsr", "rslb")]
pub async fn rankedscoreranking(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    ranking_main(GameMode::STD, RankingType::Score, ctx, msg, args).await
}

#[command]
#[short_desc("Display the global osu!mania ranked score ranking")]
#[aliases("rsrm", "rslbm")]
pub async fn rankedscorerankingmania(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
) -> BotResult<()> {
    ranking_main(GameMode::MNA, RankingType::Score, ctx, msg, args).await
}

#[command]
#[short_desc("Display the global osu!taiko ranked score ranking")]
#[aliases("rsrt", "rslbt")]
pub async fn rankedscorerankingtaiko(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
) -> BotResult<()> {
    ranking_main(GameMode::TKO, RankingType::Score, ctx, msg, args).await
}

#[command]
#[short_desc("Display the global osu!ctb ranked score ranking")]
#[aliases("rsrc", "rslbc")]
pub async fn rankedscorerankingctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    ranking_main(GameMode::CTB, RankingType::Score, ctx, msg, args).await
}

#[derive(Copy, Clone)]
pub enum RankingType {
    Performance,
    Score,
}

impl RankingType {
    #[inline]
    fn url_type(self) -> &'static str {
        match self {
            RankingType::Performance => "performance",
            RankingType::Score => "score",
        }
    }

    #[inline]
    fn title(self, country: Option<&str>) -> Cow<'static, str> {
        match (self, country) {
            (RankingType::Performance, None) => "Performance".into(),
            (RankingType::Performance, Some(country)) => format!(
                "{name}'{plural} Performance",
                name = country,
                plural = if country.ends_with('s') { "" } else { "s" }
            )
            .into(),
            (RankingType::Score, _) => "Ranked Score".into(),
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
