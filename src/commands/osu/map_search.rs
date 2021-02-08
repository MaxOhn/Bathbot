use crate::{
    arguments::Args,
    custom_client::{BeatconnectMapSet, BeatconnectMapStatus, BeatconnectSearchParams},
    embeds::{EmbedData, MapSearchEmbed},
    pagination::{MapSearchPagination, Pagination},
    unwind_error,
    util::{constants::BEATCONNECT_ISSUE, MessageExt},
    BotResult, Context,
};

use cow_utils::CowUtils;
use rosu::model::GameMode;
use std::{collections::BTreeMap, fmt::Write, sync::Arc};
use twilight_model::channel::Message;

async fn search_main(
    mode: Option<GameMode>,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let mut args: Vec<_> = args.take_all().collect();

    let status_opt = args.iter().position(|&arg| arg == "-s" || arg == "-status");

    let status = match status_opt {
        Some(idx) => {
            let arg = args.get(idx + 1).map(|arg| arg.cow_to_lowercase());

            let status = match arg.as_ref().map(|arg| arg.as_ref()) {
                Some("ranked") => Some(BeatconnectMapStatus::Ranked),
                Some("loved") => Some(BeatconnectMapStatus::Loved),
                Some("qualified") => Some(BeatconnectMapStatus::Qualified),
                Some("approved") => Some(BeatconnectMapStatus::Approved),
                Some("all") => Some(BeatconnectMapStatus::All),
                Some("unranked") | Some("wip") | Some("graveyard") | Some("pending") => {
                    Some(BeatconnectMapStatus::Unranked)
                }
                Some(_) => {
                    let content = "After the `-status` flag you must specify either \
                    `approved`, `loved`, `qualified`, `ranked`, `unranked`, or `all`.";

                    return msg.error(&ctx, content).await;
                }
                _ => None,
            };

            if status.is_some() {
                args.remove(idx);
                args.remove(idx);
            }

            status
        }
        None => None,
    };

    let mut args = args.into_iter();

    let mut query = match args.next() {
        Some(arg) => arg.to_owned(),
        None => {
            let content = "You must add a search query, e.g. a song title or mapper name.";

            return msg.error(&ctx, content).await;
        }
    };

    for arg in args {
        let _ = write!(query, " {}", arg);
    }

    let mut params = BeatconnectSearchParams::new(&query);

    if let Some(mode) = mode {
        params.mode(mode);
    }

    if let Some(status) = status {
        params.status(status);
    }

    let search_response = match ctx.clients.custom.beatconnect_search(&params).await {
        Ok(response) => response,
        Err(why) => {
            let _ = msg.error(&ctx, BEATCONNECT_ISSUE);

            return Err(why.into());
        }
    };

    let is_last_page = search_response.is_last_page();

    // Accumulate all necessary data
    let total_pages = if is_last_page {
        Some(search_response.mapsets.len() / 10 + 1)
    } else {
        None
    };

    let maps: BTreeMap<usize, BeatconnectMapSet> =
        search_response.mapsets.into_iter().enumerate().collect();

    let data = MapSearchEmbed::new(&maps, query.as_str(), (1, total_pages)).await;

    // Creating the embed
    let embed = data.build().build()?;

    let response = ctx
        .http
        .create_message(msg.channel_id)
        .embed(embed)?
        .await?;

    // Skip pagination if too few entries
    if maps.len() <= 10 {
        response.reaction_delete(&ctx, msg.author.id);

        return Ok(());
    }

    // Pagination
    let pagination =
        MapSearchPagination::new(Arc::clone(&ctx), response, maps, is_last_page, params);

    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (mapsearch): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Search for mapsets of any mode")]
#[long_desc(
    "Search for mapsets of any mode.\n\
    You can specify the rankings status with `-status` followed by \
    `approved`, `loved`, `qualified`, `ranked`, `unranked`, or `all`.\n\
    If no status is specified, `ranked` is the default.\n\
    The rest of the arguments will be considered as your search query.\n\
    Your query can contain the song title, artist, creator, ID, or tags.\n\
    Use the mode-specific command to specify a mode, \
    e.g. `searchosu` for osu!standard.\n\
    All data originates from [beatconnect.io](https://beatconnect.io/)."
)]
#[usage("[-status ranked/unranked/loved/.../all] [search query]")]
#[example("big black", "-status loved goodbye moon", "hatsune miku -status all")]
pub async fn search(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    search_main(None, ctx, msg, args).await
}

#[command]
#[short_desc("Search for osu! mapsets")]
#[long_desc(
    "Search for osu! mapsets.\n\
    You can specify the rankings status with `-status` followed by \
    `approved`, `loved`, `qualified`, `ranked`, `unranked`, or `all`.\n\
    If no status is specified, `ranked` is the default.\n\
    The rest of the arguments will be considered as your search query.\n\
    Your query can contain the song title, artist, creator, ID, or tags.\n\
    All data originates from [beatconnect.io](https://beatconnect.io/)."
)]
#[usage("[-status ranked/unranked/loved/.../all] [search query]")]
#[example("big black", "-status loved goodbye moon", "hatsune miku -status all")]
#[aliases("so")]
pub async fn searchosu(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    search_main(Some(GameMode::STD), ctx, msg, args).await
}

#[command]
#[short_desc("Search for mania mapsets")]
#[long_desc(
    "Search for mania mapsets.\n\
    You can specify the rankings status with `-status` followed by \
    `approved`, `loved`, `qualified`, `ranked`, `unranked`, or `all`.\n\
    If no status is specified, `ranked` is the default.\n\
    The rest of the arguments will be considered as your search query.\n\
    Your query can contain the song title, artist, creator, ID, or tags.\n\
    All data originates from [beatconnect.io](https://beatconnect.io/)."
)]
#[usage("[-status ranked/unranked/loved/.../all] [search query]")]
#[example("big black", "-status loved goodbye moon", "hatsune miku -status all")]
#[aliases("sm")]
pub async fn searchmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    search_main(Some(GameMode::MNA), ctx, msg, args).await
}

#[command]
#[short_desc("Search for taiko mapsets")]
#[long_desc(
    "Search for taiko mapsets.\n\
    You can specify the rankings status with `-status` followed by \
    `approved`, `loved`, `qualified`, `ranked`, `unranked`, or `all`.\n\
    If no status is specified, `ranked` is the default.\n\
    The rest of the arguments will be considered as your search query.\n\
    Your query can contain the song title, artist, creator, ID, or tags.\n\
    All data originates from [beatconnect.io](https://beatconnect.io/)."
)]
#[usage("[-status ranked/unranked/loved/.../all] [search query]")]
#[example("big black", "-status loved goodbye moon", "hatsune miku -status all")]
#[aliases("st")]
pub async fn searchtaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    search_main(Some(GameMode::TKO), ctx, msg, args).await
}

#[command]
#[short_desc("Search for ctb mapsets")]
#[long_desc(
    "Search for ctb mapsets.\n\
    You can specify the rankings status with `-status` followed by \
    `approved`, `loved`, `qualified`, `ranked`, `unranked`, or `all`.\n\
    If no status is specified, `ranked` is the default.\n\
    The rest of the arguments will be considered as your search query.\n\
    Your query can contain the song title, artist, creator, ID, or tags.\n\
    All data originates from [beatconnect.io](https://beatconnect.io/)."
)]
#[usage("[-status ranked/unranked/loved/.../all] [search query]")]
#[example("big black", "-status loved goodbye moon", "hatsune miku -status all")]
#[aliases("sc")]
pub async fn searchctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    search_main(Some(GameMode::CTB), ctx, msg, args).await
}
