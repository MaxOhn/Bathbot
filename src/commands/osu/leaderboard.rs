use crate::{
    arguments::{Args, MapModArgs},
    bail,
    embeds::{EmbedData, LeaderboardEmbed},
    pagination::{LeaderboardPagination, Pagination},
    util::{
        constants::{AVATAR_URL, GENERAL_ISSUE, OSU_API_ISSUE, OSU_WEB_ISSUE},
        osu::{map_id_from_history, MapIdType, ModSelection},
        MessageExt,
    },
    BotResult, Context,
};

use rosu::backend::requests::BeatmapRequest;
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
    } else {
        let msg_fut = ctx.http.channel_messages(msg.channel_id).limit(50).unwrap();
        let msgs = match msg_fut.await {
            Ok(msgs) => msgs,
            Err(why) => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;
                bail!("error while retrieving messages: {}", why);
            }
        };
        match map_id_from_history(&ctx, msgs).await {
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
        Err(_) => {
            let map_req = BeatmapRequest::new().map_id(map_id);
            match map_req.queue_single(ctx.osu()).await {
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
            }
        }
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
            return Err(why);
        }
    };
    let amount = scores.len();

    // Accumulate all necessary data
    let first_place_icon = scores
        .first()
        .map(|s| format!("{}{}", AVATAR_URL, s.user_id));
    let data = match LeaderboardEmbed::new(
        &ctx,
        author_name.as_deref(),
        &map,
        if scores.is_empty() {
            None
        } else {
            Some(scores.iter().take(10))
        },
        &first_place_icon,
        0,
    )
    .await
    {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            bail!("error while  creating embed: {}", why);
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
        warn!("Could not add map to DB: {}", why);
    }

    // Skip pagination if too few entries
    if scores.len() <= 10 {
        response.reaction_delete(&ctx, msg.author.id);
        return Ok(());
    }

    // Pagination
    let pagination = LeaderboardPagination::new(
        ctx.clone(),
        response,
        map,
        scores,
        author_name,
        first_place_icon,
    );
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            warn!("Pagination error (leaderboard): {}", why)
        }
    });
    Ok(())
}

#[command]
#[short_desc("Display the belgian leaderboard of a map")]
#[long_desc(
    "Display the belgian leaderboard of a given map. \
     If no map is given, I will choose the last map \
     I can find in my embeds of this channel"
)]
#[usage("[map url / map id]")]
#[example("2240404", "https://osu.ppy.sh/beatmapsets/902425#osu/2240404")]
#[aliases("lb")]
pub async fn leaderboard(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    leaderboard_main(true, ctx, msg, args).await
}

#[command]
#[short_desc("Display the global leaderboard of a map")]
#[long_desc(
    "Display the global leaderboard of a given map. \
     If no map is given, I will choose the last map \
     I can find in my embeds of this channel"
)]
#[usage("[map url / map id]")]
#[example("2240404", "https://osu.ppy.sh/beatmapsets/902425#osu/2240404")]
#[aliases("glb")]
pub async fn globalleaderboard(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    leaderboard_main(false, ctx, msg, args).await
}
