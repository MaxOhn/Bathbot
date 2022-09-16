use std::{borrow::Cow, sync::Arc};

use command_macros::command;
use eyre::Result;
use hashbrown::HashMap;
use rosu_v2::model::GameMode;

use crate::{
    commands::GameModeOption,
    core::commands::{prefix::Args, CommandOrigin},
    custom_client::OsuStatsPlayer,
    pagination::OsuStatsListPagination,
    util::{
        constants::{GENERAL_ISSUE, OSUSTATS_API_ISSUE},
        hasher::SimpleBuildHasher,
        ChannelExt, CountryCode, CowUtils,
    },
    Context,
};

use super::OsuStatsPlayers;

pub struct OsuStatsPlayersArgs {
    pub mode: GameMode,
    pub country: Option<CountryCode>,
    pub page: usize,
    pub min_rank: u32,
    pub max_rank: u32,
}

impl<'a> From<OsuStatsPlayers<'a>> for OsuStatsPlayersArgs {
    fn from(args: OsuStatsPlayers<'a>) -> Self {
        Self {
            mode: args.mode.map_or(GameMode::Osu, GameMode::from),
            country: args.country.map(|c| c.into()),
            page: 1,
            min_rank: args.min_rank.unwrap_or(OsuStatsPlayers::MIN_RANK),
            max_rank: args.max_rank.unwrap_or(OsuStatsPlayers::MAX_RANK),
        }
    }
}

pub(super) async fn players(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    mut args: OsuStatsPlayers<'_>,
) -> Result<()> {
    let owner = orig.user_id()?;

    if matches!(args.mode, None) {
        args.mode = match ctx.user_config(owner).await {
            Ok(config) => config.mode.map(GameModeOption::from),
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err.wrap_err("failed to get user config"));
            }
        };
    }

    let mut params = OsuStatsPlayersArgs::from(args);

    if let Some(country) = params.country.as_mut() {
        if country.len() != 2 {
            match CountryCode::from_name(&*country) {
                Some(code) => *country = code,
                None => {
                    let content = format!(
                        "Looks like `{country}` is neither a country name nor a country code"
                    );

                    return orig.error(&ctx, content).await;
                }
            }
        }
    }

    // Retrieve leaderboard
    let (amount, players) = match prepare_players(&ctx, &mut params).await {
        Ok(tuple) => tuple,
        Err(err) => {
            let _ = orig.error(&ctx, OSUSTATS_API_ISSUE).await;

            return Err(err.wrap_err("failed to prepare players"));
        }
    };

    let country = params
        .country
        .as_ref()
        .map(|code| code.as_str())
        .unwrap_or("Global");

    if players.is_empty() {
        let content = format!(
            "No entries found for country `{country}`.\n\
            Be sure to specify it with its acronym, e.g. `de` for germany."
        );

        return orig.error(&ctx, content).await;
    }

    let first_place_id = players[&1].first().unwrap().user_id;

    let content = format!(
        "Country: `{country}` ~ `Rank: {rank_min} - {rank_max}`",
        rank_min = params.min_rank,
        rank_max = params.max_rank,
    );

    OsuStatsListPagination::builder(players, params, first_place_id, amount)
        .content(content)
        .start_by_update()
        .defer_components()
        .start(ctx, orig)
        .await
}

/// Explicit binary search
/// 1 -> 10 -> 5
///   <: 3
///     <: 2
///     >: 4
///   >: 7
///     <: 6
///     >: 8 -> 9
///
/// If there are none, then only one request will be made.
/// Otherwise, chances are there are at least 150 entries, so two requests will be made.
/// If there are fewer than 150 people, binary search will attempt to find the exact amount
/// with as few requests as possible with a worst case of six requests (1,10,5,7,8,9).
async fn prepare_players(
    ctx: &Context,
    params: &mut OsuStatsPlayersArgs,
) -> Result<(
    usize,
    HashMap<usize, Vec<OsuStatsPlayer>, SimpleBuildHasher>,
)> {
    let mut players = HashMap::with_capacity_and_hasher(2, SimpleBuildHasher);

    // Retrieve page one
    let page = ctx.client().get_country_globals(params).await?;

    let len = page.len();

    insert(&mut players, 1, page);

    if len < 15 {
        return Ok((len, players));
    }

    // Retrieve page ten
    params.page = 10;
    let page = ctx.client().get_country_globals(params).await?;
    let len = page.len();
    insert(&mut players, 10, page);

    if len > 0 {
        return Ok((135 + len, players));
    }

    // Retrieve page five
    params.page = 5;
    let page = ctx.client().get_country_globals(params).await?;
    let len = page.len();
    insert(&mut players, 5, page);

    if 0 < len && len < 15 {
        return Ok((60 + len, players));
    } else if len == 0 {
        // Retrieve page three
        params.page = 3;
        let page = ctx.client().get_country_globals(params).await?;
        let len = page.len();
        insert(&mut players, 3, page);

        if 0 < len && len < 15 {
            return Ok((30 + len, players));
        } else if len == 0 {
            // Retrieve page two
            params.page = 2;
            let page = ctx.client().get_country_globals(params).await?;
            let len = page.len();
            insert(&mut players, 2, page);

            return Ok((15 + len, players));
        } else if len == 15 {
            // Retrieve page four
            params.page = 4;
            let page = ctx.client().get_country_globals(params).await?;
            let len = page.len();
            insert(&mut players, 4, page);

            return Ok((45 + len, players));
        }
    } else if len == 15 {
        // Retrieve page seven
        params.page = 7;
        let page = ctx.client().get_country_globals(params).await?;
        let len = page.len();
        insert(&mut players, 7, page);

        if 0 < len && len < 15 {
            return Ok((90 + len, players));
        } else if len == 0 {
            // Retrieve page six
            params.page = 6;
            let page = ctx.client().get_country_globals(params).await?;
            let len = page.len();
            insert(&mut players, 6, page);

            return Ok((75 + len, players));
        }
    }

    for idx in 8..=9 {
        // Retrieve page idx
        params.page = idx;
        let page = ctx.client().get_country_globals(params).await?;
        let len = page.len();
        insert(&mut players, idx, page);

        if len < 15 {
            return Ok(((idx - 1) * 15 + len, players));
        }
    }

    Ok((120 + len, players))
}

fn insert(
    map: &mut HashMap<usize, Vec<OsuStatsPlayer>, SimpleBuildHasher>,
    page: usize,
    players: Vec<OsuStatsPlayer>,
) {
    if !players.is_empty() {
        map.insert(page, players);
    }
}

#[command]
#[desc("National leaderboard of global leaderboard counts")]
#[help(
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
#[examples("rankr=42 be", "rank=1..5", "fr")]
#[aliases("osl")]
#[group(Osu)]
async fn prefix_osustatslist(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match OsuStatsPlayers::args(None, args) {
        Ok(args) => players(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("National leaderboard of global mania leaderboard counts")]
#[help(
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
#[examples("rankr=42 be", "rank=1..5", "fr")]
#[aliases("oslm")]
#[group(Mania)]
async fn prefix_osustatslistmania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match OsuStatsPlayers::args(Some(GameModeOption::Mania), args) {
        Ok(args) => players(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("National leaderboard of global taiko leaderboard counts")]
#[help(
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
#[examples("rankr=42 be", "rank=1..5", "fr")]
#[aliases("oslt")]
#[group(Taiko)]
async fn prefix_osustatslisttaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match OsuStatsPlayers::args(Some(GameModeOption::Taiko), args) {
        Ok(args) => players(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("National leaderboard of global ctb leaderboard counts")]
#[help(
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
#[examples("rankr=42 be", "rank=1..5", "fr")]
#[aliases("oslc", "osustatslistcatch")]
#[group(Catch)]
async fn prefix_osustatslistctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match OsuStatsPlayers::args(Some(GameModeOption::Catch), args) {
        Ok(args) => players(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

impl<'m> OsuStatsPlayers<'m> {
    const MIN_RANK: u32 = 1;
    const MAX_RANK: u32 = 100;

    const ERR_PARSE_RANK: &'static str = "Failed to parse `rank`.\n\
        Must be either a positive integer \
        or two positive integers of the form `a..b` e.g. `2..45`.";

    fn args(mode: Option<GameModeOption>, args: Args<'m>) -> Result<Self, Cow<'static, str>> {
        let mut country = None;
        let mut min_rank = None;
        let mut max_rank = None;

        for arg in args.take(2).map(|arg| arg.cow_to_ascii_lowercase()) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "rank" | "r" => match value.find("..") {
                        Some(idx) => {
                            let bot = &value[..idx];
                            let top = &value[idx + 2..];

                            let min = if bot.is_empty() {
                                Self::MIN_RANK
                            } else if let Ok(num) = bot.parse::<u32>() {
                                num.max(Self::MIN_RANK).min(Self::MAX_RANK)
                            } else {
                                return Err(Self::ERR_PARSE_RANK.into());
                            };

                            let max = if top.is_empty() {
                                Self::MAX_RANK
                            } else if let Ok(num) = top.parse::<u32>() {
                                num.max(Self::MIN_RANK).min(Self::MAX_RANK)
                            } else {
                                return Err(Self::ERR_PARSE_RANK.into());
                            };

                            min_rank = Some(min.min(max));
                            max_rank = Some(min.max(max));
                        }
                        None => max_rank = Some(value.parse().map_err(|_| Self::ERR_PARSE_RANK)?),
                    },
                    _ => {
                        let content =
                            format!("Unrecognized option `{key}`.\nAvailable options are: `rank`.");

                        return Err(content.into());
                    }
                }
            } else if arg.len() == 2 && arg.is_ascii() {
                country = Some(arg);
            } else if let Some(code) = CountryCode::from_name(&arg) {
                country = Some(code.as_str().to_owned().into());
            } else {
                let content = format!(
                    "Failed to parse `{arg}` as either rank or country.\n\
                    Be sure to specify valid country or two ASCII letter country code.\n\
                    A rank range can be specified like `rank=2..45`."
                );

                return Err(content.into());
            }
        }

        Ok(Self {
            mode,
            country,
            min_rank,
            max_rank,
        })
    }
}
