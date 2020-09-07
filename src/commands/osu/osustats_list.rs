use crate::{
    arguments::{Args, OsuStatsListArgs},
    custom_client::{OsuStatsListParams, OsuStatsPlayer},
    embeds::{EmbedData, OsuStatsListEmbed},
    pagination::{OsuStatsListPagination, Pagination},
    util::{numbers, MessageExt},
    BotResult, Context,
};

use rosu::models::GameMode;
use std::{collections::HashMap, sync::Arc};
use twilight::model::channel::Message;

async fn osustats_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    // Parse arguments
    let mut params = match OsuStatsListArgs::new(args, mode) {
        Ok(args) => args.params,
        Err(err_msg) => return msg.error(&ctx, err_msg).await,
    };

    // Retrieve leaderboard
    let (amount, players) = match prepare_players(&ctx, &mut params).await {
        Ok(tuple) => tuple,
        Err(why) => {
            let content = "Some issue with the osustats website, blame bade";
            let _ = msg.error(&ctx, content).await;
            return Err(why);
        }
    };
    if players.is_empty() {
        let country = params.country.as_deref().unwrap_or("Global");
        let content = format!(
            "No entries found for country `{}`.\n\
            Be sure to specify it with its acronym, e.g. `de` for germany.",
            country
        );
        return msg.error(&ctx, content).await;
    }

    // Accumulate all necessary data
    let pages = numbers::div_euclid(15, amount);
    let first_place_id = players[&1].first().unwrap().user_id;
    let data = OsuStatsListEmbed::new(&players[&1], &params.country, first_place_id, (1, pages));
    let content = format!(
        "Country: `{country}` ~ `Rank: {rank_min} - {rank_max}`",
        country = params.country.as_deref().unwrap_or("Global"),
        rank_min = params.rank_min,
        rank_max = params.rank_max,
    );

    // Creating the embed
    let embed = data.build().build()?;
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .content(content)?
        .embed(embed)?
        .await?;

    // Skip pagination if too few entries
    if players.len() <= 1 {
        response.reaction_delete(&ctx, msg.author.id);
        return Ok(());
    }

    // Pagination
    let pagination = OsuStatsListPagination::new(ctx.clone(), response, players, params, amount);
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            warn!("Pagination error (osustatslist): {}", why)
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
    let page = ctx.clients.custom.get_country_globals(&params).await?;
    let len = page.len();
    insert(&mut players, 1, page);
    if len < 15 {
        return Ok((len, players));
    }
    // Retrieve page ten
    params.page = 10;
    let page = ctx.clients.custom.get_country_globals(&params).await?;
    let len = page.len();
    insert(&mut players, 10, page);
    if len > 0 {
        return Ok((135 + len, players));
    }
    // Retrieve page five
    params.page = 5;
    let page = ctx.clients.custom.get_country_globals(&params).await?;
    let len = page.len();
    insert(&mut players, 5, page);
    if 0 < len && len < 15 {
        return Ok((60 + len, players));
    } else if len == 0 {
        // Retrieve page three
        params.page = 3;
        let page = ctx.clients.custom.get_country_globals(&params).await?;
        let len = page.len();
        insert(&mut players, 3, page);
        if 0 < len && len < 15 {
            return Ok((30 + len, players));
        } else if len == 0 {
            // Retrieve page two
            params.page = 2;
            let page = ctx.clients.custom.get_country_globals(&params).await?;
            let len = page.len();
            insert(&mut players, 2, page);
            return Ok((15 + len, players));
        } else if len == 15 {
            // Retrieve page four
            params.page = 4;
            let page = ctx.clients.custom.get_country_globals(&params).await?;
            let len = page.len();
            insert(&mut players, 4, page);
            return Ok((45 + len, players));
        }
    } else if len == 15 {
        // Retrieve page seven
        params.page = 7;
        let page = ctx.clients.custom.get_country_globals(&params).await?;
        let len = page.len();
        insert(&mut players, 7, page);
        if 0 < len && len < 15 {
            return Ok((90 + len, players));
        } else if len == 0 {
            // Retrieve page six
            params.page = 6;
            let page = ctx.clients.custom.get_country_globals(&params).await?;
            let len = page.len();
            insert(&mut players, 6, page);
            return Ok((75 + len, players));
        }
    }
    for idx in 8..=9 {
        // Retrieve page idx
        params.page = idx;
        let page = ctx.clients.custom.get_country_globals(&params).await?;
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
    The rank range can be specified with `-r` followed by either a number \
    for max rank, or two numbers of the form `a..b` for min and max rank.\n\
    The rank range default to 1..100.\n\
    To specify a country, provide its acronym, e.g. `de` for germany.\n\
    If no country is specified, I'll show the global leaderboard.\n\
    Check https://osustats.ppy.sh/r for more info."
)]
#[usage("[-r [num..]num] [country acronym]")]
#[example("-r 42 be", "-r 1..5", "fr")]
#[aliases("osl")]
pub async fn osustatslist(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("National leaderboard of global mania leaderboard counts")]
#[long_desc(
    "Display either the global or a national leaderboard of mania players, \
    sorted by their amounts of scores on a map's global leaderboard.\n\
    The rank range can be specified with `-r` followed by either a number \
    for max rank, or two numbers of the form `a..b` for min and max rank.\n\
    The rank range default to 1..100.\n\
    To specify a country, provide its acronym, e.g. `de` for germany.\n\
    If no country is specified, I'll show the global leaderboard.\n\
    Check https://osustats.ppy.sh/r for more info."
)]
#[usage("[-r [num..]num] [country acronym]")]
#[example("-r 42 be", "-r 1..5", "fr")]
#[aliases("oslm")]
pub async fn osustatslistmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("National leaderboard of global taiko leaderboard counts")]
#[long_desc(
    "Display either the global or a national leaderboard of taiko players, \
    sorted by their amounts of scores on a map's global leaderboard.\n\
    The rank range can be specified with `-r` followed by either a number \
    for max rank, or two numbers of the form `a..b` for min and max rank.\n\
    The rank range default to 1..100.\n\
    To specify a country, provide its acronym, e.g. `de` for germany.\n\
    If no country is specified, I'll show the global leaderboard.\n\
    Check https://osustats.ppy.sh/r for more info."
)]
#[usage("[-r [num..]num] [country acronym]")]
#[example("-r 42 be", "-r 1..5", "fr")]
#[aliases("oslt")]
pub async fn osustatslisttaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("National leaderboard of global ctb leaderboard counts")]
#[long_desc(
    "Display either the global or a national leaderboard of ctb players, \
    sorted by their amounts of scores on a map's global leaderboard.\n\
    The rank range can be specified with `-r` followed by either a number \
    for max rank, or two numbers of the form `a..b` for min and max rank.\n\
    The rank range default to 1..100.\n\
    To specify a country, provide its acronym, e.g. `de` for germany.\n\
    If no country is specified, I'll show the global leaderboard.\n\
    Check https://osustats.ppy.sh/r for more info."
)]
#[usage("[-r [num..]num] [country acronym]")]
#[example("-r 42 be", "-r 1..5", "fr")]
#[aliases("oslc")]
pub async fn osustatslistctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::CTB, ctx, msg, args).await
}
