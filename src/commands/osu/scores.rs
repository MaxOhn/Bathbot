use super::require_link;
use crate::{
    arguments::{Args, NameMapArgs},
    embeds::{EmbedData, ScoresEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        osu::{map_id_from_history, MapIdType},
        MessageExt,
    },
    BotResult, Context,
};

use rosu::backend::requests::{BeatmapRequest, ScoreRequest, UserRequest};
use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Each mod's top score from a player on a map")]
#[long_desc(
    "Display a user's top score for each mod on a given map. \
     If no map is given, I will choose the last map \
     I can find in my embeds of this channel"
)]
#[usage("[username] [map url / map id]")]
#[example("badewanne3")]
#[example("badewanne3 2240404")]
#[example("badewanne3 https://osu.ppy.sh/beatmapsets/902425#osu/2240404")]
#[aliases("c", "compare")]
async fn scores(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = NameMapArgs::new(args);
    let map_id = if let Some(id) = args.map_id {
        id.id()
    } else {
        let msg_fut = ctx.http.channel_messages(msg.channel_id).limit(50).unwrap();
        let msgs = match msg_fut.await {
            Ok(msgs) => msgs,
            Err(why) => {
                msg.respond(&ctx, "Error while retrieving messages").await?;
                return Err(why.into());
            }
        };
        match map_id_from_history(&ctx, msgs).await {
            Some(MapIdType::Map(id)) => id,
            Some(MapIdType::Set(_)) => {
                let content = "Looks like you gave me a mapset id, I need a map id though";
                return msg.respond(&ctx, content).await;
            }
            None => {
                let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";
                return msg.respond(&ctx, content).await;
            }
        }
    };
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return require_link(&ctx, msg).await,
    };

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
                    return msg.respond(&ctx, content).await;
                }
                Err(why) => {
                    msg.respond(&ctx, OSU_API_ISSUE).await?;
                    return Err(why.into());
                }
            }
        }
    };

    // Retrieve user and user's scores on the map
    let score_req = ScoreRequest::with_map_id(map_id)
        .username(&name)
        .mode(map.mode);
    let join_result = tokio::try_join!(ctx.osu_user(&name, map.mode), score_req.queue(ctx.osu()));
    let (user, scores) = match join_result {
        Ok((Some(user), scores)) => (user, scores),
        Ok((None, _)) => {
            let content = format!("Could not find user `{}`", name);
            return msg.respond(&ctx, content).await;
        }
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why.into());
        }
    };

    // Accumulate all necessary data
    let data = match ScoresEmbed::new(&ctx, user, &map, scores).await {
        Ok(data) => data,
        Err(why) => {
            msg.respond(&ctx, GENERAL_ISSUE).await?;
            return Err(why);
        }
    };

    // Sending the embed
    let embed = data.build().build();
    msg.build_response(&ctx, |m| m.embed(embed)).await?;

    // Add map to database if its not in already
    if let Err(why) = ctx.clients.psql.insert_beatmap(&map).await {
        warn!("Error while adding new map to DB: {}", why);
    }
    Ok(())
}
