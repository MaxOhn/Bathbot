use crate::{
    arguments::{Args, MapModArgs},
    embeds::{EmbedData, LeaderboardEmbed},
    pagination::{LeaderboardPagination, Pagination},
    unwind_error,
    util::{
        constants::{AVATAR_URL, GENERAL_ISSUE, OSU_API_ISSUE, OSU_WEB_ISSUE},
        osu::{cached_message_extract, map_id_from_history, MapIdType, ModSelection},
        MessageExt,
    },
    BotResult, Context,
};

use std::sync::Arc;
use twilight_model::channel::Message;

#[allow(clippy::cognitive_complexity)]
async fn leaderboard_main(
    national: bool,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let author_name = ctx.get_link(msg.author.id.0);
    let args = MapModArgs::new(args);

    let map_id = if let Some(id) = args.map_id {
        match id {
            MapIdType::Map(id) => id,
            MapIdType::Set(_) => {
                let content = "Looks like you gave me a mapset id, I need a map id though";

                return msg.error(&ctx, content).await;
            }
        }
    } else if let Some(id) = ctx
        .cache
        .message_extract(msg.channel_id, cached_message_extract)
    {
        id.id()
    } else {
        let msgs = match ctx.retrieve_channel_history(msg.channel_id).await {
            Ok(msgs) => msgs,
            Err(why) => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                return Err(why.into());
            }
        };

        match map_id_from_history(msgs) {
            Some(MapIdType::Map(id)) => id,
            Some(MapIdType::Set(_)) => {
                let content = "Looks like you gave me a mapset id, I need a map id though";

                return msg.error(&ctx, content).await;
            }
            None => {
                let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";

                return msg.error(&ctx, content).await;
            }
        }
    };
    let selection = args.mods;

    // Retrieving the beatmap
    let map = match ctx.psql().get_beatmap(map_id).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(Some(map)) => map,
            Ok(None) => {
                let content = format!(
                    "Could not find beatmap with id `{}`. \
                        Did you give me a mapset id instead of a map id?",
                    map_id
                );

                return msg.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        },
    };

    // Retrieve the map's leaderboard
    let scores_future = ctx.clients.custom.get_leaderboard(
        map_id,
        national,
        match selection {
            Some(ModSelection::Exclude(_)) | None => None,
            Some(ModSelection::Include(m)) | Some(ModSelection::Exact(m)) => Some(m),
        },
        map.mode,
    );

    let scores = match scores_future.await {
        Ok(scores) => scores,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_WEB_ISSUE).await;

            return Err(why.into());
        }
    };

    let amount = scores.len();

    // Accumulate all necessary data
    let first_place_icon = scores
        .first()
        .map(|s| format!("{}{}", AVATAR_URL, s.user_id));

    let data_fut = LeaderboardEmbed::new(
        author_name.as_deref(),
        &map,
        if scores.is_empty() {
            None
        } else {
            Some(scores.iter().take(10))
        },
        &first_place_icon,
        0,
    );

    let data = match data_fut.await {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Sending the embed
    let embed = data.build().build()?;

    let content = format!(
        "I found {} scores with the specified mods on the map's leaderboard",
        amount
    );

    let response = ctx
        .http
        .create_message(msg.channel_id)
        .content(content)?
        .embed(embed)?
        .await?;

    // Add map to database if its not in already
    if let Err(why) = ctx.psql().insert_beatmap(&map).await {
        unwind_error!(warn, why, "Could not add map to DB: {}");
    }

    // Set map on garbage collection list if unranked
    let gb = ctx.map_garbage_collector(&map);

    // Skip pagination if too few entries
    if scores.len() <= 10 {
        response.reaction_delete(&ctx, msg.author.id);

        return Ok(());
    }

    // Pagination
    let pagination =
        LeaderboardPagination::new(response, map, scores, author_name, first_place_icon);
    let owner = msg.author.id;

    gb.execute(&ctx).await;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (leaderboard): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display the global leaderboard of a map")]
#[long_desc(
    "Display the global leaderboard of a given map.\n\
     If no map is given, I will choose the last map \
     I can find in the embeds of this channel.\n\
     Mods can be specified."
)]
#[usage("[map url / map id] [mods]")]
#[example("2240404", "https://osu.ppy.sh/beatmapsets/902425#osu/2240404")]
#[aliases("lb", "glb", "globalleaderboard")]
pub async fn leaderboard(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    leaderboard_main(false, ctx, msg, args).await
}

#[command]
#[short_desc("Display the belgian leaderboard of a map")]
#[long_desc(
    "Display the belgian leaderboard of a given map.\n\
     If no map is given, I will choose the last map \
     I can find in the embeds of this channel.\n\
     Mods can be specified."
)]
#[usage("[map url / map id] [mods]")]
#[example("2240404", "https://osu.ppy.sh/beatmapsets/902425#osu/2240404")]
#[aliases("blb")]
pub async fn belgianleaderboard(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    leaderboard_main(true, ctx, msg, args).await
}
