use crate::{
    commands::parse_mode_option,
    custom_client::{OsuStatsListParams, OsuStatsPlayer},
    embeds::{EmbedData, OsuStatsListEmbed},
    error::Error,
    pagination::{OsuStatsListPagination, Pagination},
    util::{
        constants::{
            common_literals::{COUNTRY, MODE, RANK},
            GENERAL_ISSUE, OSUSTATS_API_ISSUE,
        },
        numbers, CountryCode, MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder,
};

use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::model::GameMode;
use std::sync::Arc;
use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};

pub(super) async fn _players(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    mut params: OsuStatsListParams,
) -> BotResult<()> {
    let owner = data.author()?.id;

    if params.mode == GameMode::STD {
        params.mode = match ctx.user_config(owner).await {
            Ok(config) => config.mode.unwrap_or(GameMode::STD),
            Err(why) => {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }
        };
    }

    // Retrieve leaderboard
    let (amount, players) = match prepare_players(&ctx, &mut params).await {
        Ok(tuple) => tuple,
        Err(why) => {
            let _ = data.error(&ctx, OSUSTATS_API_ISSUE).await;

            return Err(why);
        }
    };

    let country = params
        .country
        .as_ref()
        .map(|code| code.as_str())
        .unwrap_or("Global");

    if players.is_empty() {
        let content = format!(
            "No entries found for country `{}`.\n\
            Be sure to specify it with its acronym, e.g. `de` for germany.",
            country
        );

        return data.error(&ctx, content).await;
    }

    // Accumulate all necessary data
    let pages = numbers::div_euclid(15, amount);
    let first_place_id = players[&1].first().unwrap().user_id;
    let embed_data =
        OsuStatsListEmbed::new(&players[&1], &params.country, first_place_id, (1, pages));

    let content = format!(
        "Country: `{country}` ~ `Rank: {rank_min} - {rank_max}`",
        country = country,
        rank_min = params.rank_min,
        rank_max = params.rank_max,
    );

    // Creating the embed
    let embed = embed_data.into_builder().build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    let response_raw = data.create_message(&ctx, builder).await?;

    // Skip pagination if too few entries
    if players.len() <= 1 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination =
        OsuStatsListPagination::new(Arc::clone(&ctx), response, players, params, amount);

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

// Explicit binary search
// 1 -> 10 -> 5
//   <: 3
//     <: 2
//     >: 4
//   >: 7
//     <: 6
//     >: 8 -> 9
//
// If there are none, then only one request will be made.
// Otherwise, chances are there are at least 150 entries, so two requests will be made.
// If there are fewer than 150 people, binary search will attempt to find the exact amount
// with as few requests as possible with a worst case of six requests (1,10,5,7,8,9).
async fn prepare_players(
    ctx: &Context,
    params: &mut OsuStatsListParams,
) -> BotResult<(usize, HashMap<usize, Vec<OsuStatsPlayer>>)> {
    let mut players = HashMap::with_capacity(2);

    // Retrieve page one
    let page = ctx.clients.custom.get_country_globals(params).await?;
    let len = page.len();

    insert(&mut players, 1, page);

    if len < 15 {
        return Ok((len, players));
    }

    // Retrieve page ten
    params.page = 10;
    let page = ctx.clients.custom.get_country_globals(params).await?;
    let len = page.len();
    insert(&mut players, 10, page);

    if len > 0 {
        return Ok((135 + len, players));
    }

    // Retrieve page five
    params.page = 5;
    let page = ctx.clients.custom.get_country_globals(params).await?;
    let len = page.len();
    insert(&mut players, 5, page);

    if 0 < len && len < 15 {
        return Ok((60 + len, players));
    } else if len == 0 {
        // Retrieve page three
        params.page = 3;
        let page = ctx.clients.custom.get_country_globals(params).await?;
        let len = page.len();
        insert(&mut players, 3, page);

        if 0 < len && len < 15 {
            return Ok((30 + len, players));
        } else if len == 0 {
            // Retrieve page two
            params.page = 2;
            let page = ctx.clients.custom.get_country_globals(params).await?;
            let len = page.len();
            insert(&mut players, 2, page);

            return Ok((15 + len, players));
        } else if len == 15 {
            // Retrieve page four
            params.page = 4;
            let page = ctx.clients.custom.get_country_globals(params).await?;
            let len = page.len();
            insert(&mut players, 4, page);

            return Ok((45 + len, players));
        }
    } else if len == 15 {
        // Retrieve page seven
        params.page = 7;
        let page = ctx.clients.custom.get_country_globals(params).await?;
        let len = page.len();
        insert(&mut players, 7, page);

        if 0 < len && len < 15 {
            return Ok((90 + len, players));
        } else if len == 0 {
            // Retrieve page six
            params.page = 6;
            let page = ctx.clients.custom.get_country_globals(params).await?;
            let len = page.len();
            insert(&mut players, 6, page);

            return Ok((75 + len, players));
        }
    }

    for idx in 8..=9 {
        // Retrieve page idx
        params.page = idx;
        let page = ctx.clients.custom.get_country_globals(params).await?;
        let len = page.len();
        insert(&mut players, idx, page);

        if len < 15 {
            return Ok(((idx - 1) * 15 + len, players));
        }
    }

    Ok((120 + len, players))
}

fn insert(
    map: &mut HashMap<usize, Vec<OsuStatsPlayer>>,
    page: usize,
    players: Vec<OsuStatsPlayer>,
) {
    if !players.is_empty() {
        map.insert(page, players);
    }
}

#[command]
#[short_desc("National leaderboard of global leaderboard counts")]
#[long_desc(
    "Display either the global or a national leaderboard of players, \
    sorted by their amounts of scores on a map's global leaderboard.\n\
    The rank range can be specified with `rank=` followed by either a number \
    for max rank, or two numbers of the form `a..b` for min and max rank.\n\
    The rank range default to 1..100.\n\
    To specify a country, provide its acronym, e.g. `de` for germany.\n\
    If no country is specified, I'll show the global leaderboard.\n\
    Check https://osustats.ppy.sh/r for more info."
)]
#[usage("[rank=[num..]num] [country acronym]")]
#[example("rankr=42 be", "rank=1..5", "fr")]
#[aliases("osl")]
pub async fn osustatslist(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match OsuStatsListParams::args(&mut args, GameMode::STD) {
                Ok(params) => _players(ctx, CommandData::Message { msg, args, num }, params).await,
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_osustats(ctx, *command).await,
    }
}

#[command]
#[short_desc("National leaderboard of global mania leaderboard counts")]
#[long_desc(
    "Display either the global or a national leaderboard of mania players, \
    sorted by their amounts of scores on a map's global leaderboard.\n\
    The rank range can be specified with `rank=` followed by either a number \
    for max rank, or two numbers of the form `a..b` for min and max rank.\n\
    The rank range default to 1..100.\n\
    To specify a country, provide its acronym, e.g. `de` for germany.\n\
    If no country is specified, I'll show the global leaderboard.\n\
    Check https://osustats.ppy.sh/r for more info."
)]
#[usage("[rank=[num..]num] [country acronym]")]
#[example("rankr=42 be", "rank=1..5", "fr")]
#[aliases("oslm")]
pub async fn osustatslistmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match OsuStatsListParams::args(&mut args, GameMode::MNA) {
                Ok(params) => _players(ctx, CommandData::Message { msg, args, num }, params).await,
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_osustats(ctx, *command).await,
    }
}

#[command]
#[short_desc("National leaderboard of global taiko leaderboard counts")]
#[long_desc(
    "Display either the global or a national leaderboard of taiko players, \
    sorted by their amounts of scores on a map's global leaderboard.\n\
    The rank range can be specified with `rank=` followed by either a number \
    for max rank, or two numbers of the form `a..b` for min and max rank.\n\
    The rank range default to 1..100.\n\
    To specify a country, provide its acronym, e.g. `de` for germany.\n\
    If no country is specified, I'll show the global leaderboard.\n\
    Check https://osustats.ppy.sh/r for more info."
)]
#[usage("[rank=[num..]num] [country acronym]")]
#[example("rankr=42 be", "rank=1..5", "fr")]
#[aliases("oslt")]
pub async fn osustatslisttaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match OsuStatsListParams::args(&mut args, GameMode::TKO) {
                Ok(params) => _players(ctx, CommandData::Message { msg, args, num }, params).await,
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_osustats(ctx, *command).await,
    }
}

#[command]
#[short_desc("National leaderboard of global ctb leaderboard counts")]
#[long_desc(
    "Display either the global or a national leaderboard of ctb players, \
    sorted by their amounts of scores on a map's global leaderboard.\n\
    The rank range can be specified with `rank=` followed by either a number \
    for max rank, or two numbers of the form `a..b` for min and max rank.\n\
    The rank range default to 1..100.\n\
    To specify a country, provide its acronym, e.g. `de` for germany.\n\
    If no country is specified, I'll show the global leaderboard.\n\
    Check https://osustats.ppy.sh/r for more info."
)]
#[usage("[rank=[num..]num] [country acronym]")]
#[example("rankr=42 be", "rank=1..5", "fr")]
#[aliases("oslc")]
pub async fn osustatslistctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match OsuStatsListParams::args(&mut args, GameMode::CTB) {
                Ok(params) => _players(ctx, CommandData::Message { msg, args, num }, params).await,
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_osustats(ctx, *command).await,
    }
}

impl OsuStatsListParams {
    const MIN_RANK: usize = 1;
    const MAX_RANK: usize = 100;

    const ERR_PARSE_RANK: &'static str = "Failed to parse `rank`.\n\
        Must be either a positive integer \
        or two positive integers of the form `a..b` e.g. `2..45`.";

    fn args(args: &mut Args<'_>, mode: GameMode) -> Result<Self, String> {
        let mut country = None;
        let mut rank_min = None;
        let mut rank_max = None;

        for arg in args.take(2) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    RANK | "r" => match value.find("..") {
                        Some(idx) => {
                            let bot = &value[..idx];
                            let top = &value[idx + 2..];

                            let min = if bot.is_empty() {
                                Self::MIN_RANK
                            } else if let Ok(num) = bot.parse::<usize>() {
                                num.max(Self::MIN_RANK).min(Self::MAX_RANK)
                            } else {
                                return Err(Self::ERR_PARSE_RANK.into());
                            };

                            let max = if top.is_empty() {
                                Self::MAX_RANK
                            } else if let Ok(num) = top.parse::<usize>() {
                                num.max(Self::MIN_RANK).min(Self::MAX_RANK)
                            } else {
                                return Err(Self::ERR_PARSE_RANK.into());
                            };

                            rank_min = Some(min.min(max));
                            rank_max = Some(min.max(max));
                        }
                        None => rank_max = Some(value.parse().map_err(|_| Self::ERR_PARSE_RANK)?),
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `rank`.",
                            key
                        );

                        return Err(content);
                    }
                }
            } else if arg.len() == 2 && arg.is_ascii() {
                country = Some(arg.into());
            } else if let Some(code) = CountryCode::from_name(arg) {
                country = Some(code);
            } else {
                let content = format!(
                    "Failed to parse `{}` as either rank or country.\n\
                    Be sure to specify valid country or two ASCII letter country code.\n\
                    A rank range can be specified like `rank=2..45`.",
                    arg
                );

                return Err(content);
            }
        }

        let params = Self {
            country,
            mode,
            page: 1,
            rank_min: rank_min.unwrap_or(Self::MIN_RANK),
            rank_max: rank_max.unwrap_or(Self::MAX_RANK),
        };

        Ok(params)
    }

    pub(super) fn slash(options: Vec<CommandDataOption>) -> BotResult<Result<Self, String>> {
        let mut country = None;
        let mut mode = None;
        let mut rank_min = None;
        let mut rank_max = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    MODE => mode = parse_mode_option(&value),
                    COUNTRY => {
                        if value.len() == 2 && value.is_ascii() {
                            country = Some(value.into())
                        } else if let Some(code) = CountryCode::from_name(value.as_str()) {
                            country = Some(code);
                        } else {
                            let content = format!(
                                "Failed to parse `{}` as country or country code.\n\
                                Be sure to specify valid country or two ASCII letter country code.",
                                value
                            );

                            return Ok(Err(content));
                        }
                    }
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Integer(value) => match option.name.as_str() {
                    "min_rank" => {
                        rank_min =
                            Some((value.max(Self::MIN_RANK as i64) as usize).min(Self::MAX_RANK))
                    }
                    "max_rank" => {
                        rank_max =
                            Some((value.max(Self::MIN_RANK as i64) as usize).min(Self::MAX_RANK))
                    }
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        let params = Self {
            country,
            mode: mode.unwrap_or(GameMode::STD),
            page: 1,
            rank_min: rank_min.unwrap_or(Self::MIN_RANK),
            rank_max: rank_max.unwrap_or(Self::MAX_RANK),
        };

        Ok(Ok(params))
    }
}
