use super::require_link;
use crate::{
    arguments::{Args, NameMapArgs},
    embeds::{EmbedData, ScoresEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
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
    let map_id = if let Some(map_id) = args.map_id {
        map_id
    } else {
        let msg_fut = ctx.http.channel_messages(msg.channel_id).limit(50).unwrap();
        let msgs = match msg_fut.await {
            Ok(msgs) => msgs,
            Err(why) => {
                msg.respond(&ctx, "Error while retrieving messages").await?;
                return Err(why.into());
            }
        };
        match discord::map_id_from_history(msgs, &ctx).await {
            Some(id) => id,
            None => {
                let content = "No map embed found in this channel's recent history.\n\
                         Try specifying a map as last argument either by url to the map, \
                         or just by map id.";
                return msg.respond(&ctx, content).await;
            }
        }
    };
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return require_link(&ctx, msg).await,
    };

    // Retrieving the beatmap
    let map = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        match mysql.get_beatmap(map_id).await {
            Ok(map) => map,
            Err(_) => {
                let map_req = BeatmapRequest::new().map_id(map_id);
                let osu = data.get::<Osu>().unwrap();
                match map_req.queue_single(&osu).await {
                    Ok(result) => match result {
                        Some(map) => map,
                        None => {
                            let content = format!(
                                "Could not find beatmap with id `{}`. \
                                Did you give me a mapset id instead of a map id?",
                                map_id
                            );
                            msg.respond(&ctx, content).await?;
                            return Ok(());
                        }
                    },
                    Err(why) => {
                        msg.respond(&ctx, OSU_API_ISSUE).await?;
                        return Err(why.into());
                    }
                }
            }
        }
    };

    // Retrieve user and user's scores on the map
    let osu = &ctx.clients.osu;
    let score_req = ScoreRequest::with_map_id(map_id)
        .username(&name)
        .mode(map.mode);
    let scores = match score_req.queue(osu).await {
        Ok(scores) => scores,
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why.into());
        }
    };
    let user = match ctx.osu_user(&name, map.mode).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let content = format!("Could not find user `{}`", name);
            return msg.respond(&ctx, content).await;
        }
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why.into());
        }
    };

    // Accumulate all necessary data
    let data = match ScoresEmbed::new(user, &map, scores, ctx).await {
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
