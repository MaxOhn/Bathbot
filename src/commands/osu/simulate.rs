use crate::{
    arguments::{Args, SimulateMapArgs},
    embeds::{EmbedData, SimulateEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        osu::{map_id_from_history, MapIdType},
        MessageExt,
    },
    BotResult, Context,
};

use rosu::{
    backend::requests::BeatmapRequest,
    models::{
        ApprovalStatus::{Approved, Loved, Ranked},
        GameMode,
    },
};
use std::sync::Arc;
use tokio::time::{self, Duration};
use twilight::model::channel::Message;

// TODO: Split up into STD / MNA commands
#[command]
#[short_desc("Simulate a score on a map")]
#[long_desc(
    "Simulate a (perfect) score on the given map. \
     If no map is given, I will choose the last map \
     I can find in my embeds of this channel.\n\
     The `-s` argument is only relevant for mania."
)]
#[usage(
    "[map url / map id] [-a acc%] [-300 #300s] [-100 #100s] [-50 #50s] [-m #misses] [-s score]"
)]
#[example("1980365 -a 99.3 -300 1422 -m 1")]
#[example("https://osu.ppy.sh/beatmapsets/948199#osu/1980365")]
#[aliases("s")]
async fn simulate(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = match SimulateMapArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => return msg.respond(&ctx, err_msg).await,
    };
    let map_id = if let Some(id) = args.map_id {
        id
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

    if let GameMode::TKO | GameMode::CTB = map.mode {
        let content = format!("I can only simulate STD and MNA maps, not {}", map.mode);
        return msg.respond(&ctx, content).await;
    }

    // Accumulate all necessary data
    let data = match SimulateEmbed::new(&ctx, None, &map, args.into()).await {
        Ok(data) => data,
        Err(why) => {
            msg.respond(&ctx, GENERAL_ISSUE).await?;
            return Err(why);
        }
    };

    // Creating the embed
    let embed = data.build().build();
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .embed(embed)?
        .await?;

    // Add map to database if its not in already
    if let Err(why) = ctx.psql().insert_beatmap(&map).await {
        warn!("Could not add map to DB: {}", why);
    }
    response.reaction_delete(&ctx, msg.author.id);

    // Minimize embed after delay
    time::delay_for(Duration::from_secs(45)).await;
    let embed = data.minimize().build();
    let edit_fut = ctx
        .http
        .update_message(response.channel_id, response.id)
        .embed(embed)?;
    if let Err(why) = edit_fut.await {
        warn!("Error while minimizing simulate msg: {}", why);
    }
    Ok(())
}
