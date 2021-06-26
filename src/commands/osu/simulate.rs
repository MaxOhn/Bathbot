use crate::{
    arguments::{Args, SimulateMapArgs},
    embeds::{EmbedData, SimulateEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        osu::{cached_message_extract, map_id_from_history, map_id_from_msg, MapIdType},
        MessageExt,
    },
    BotResult, Context,
};

use rosu_v2::prelude::{BeatmapsetCompact, OsuError};
use std::sync::Arc;
use tokio::time::{self, Duration};
use twilight_model::channel::{message::MessageType, Message};

#[command]
#[short_desc("Simulate a score on a map")]
#[long_desc(
    "Simulate a (perfect) score on the given map. \
     Mods can be specified.\n\
     If no map is given, I will choose the last map \
     I can find in the embeds of this channel.\n\
     The `-s` argument is only relevant for mania."
)]
#[usage(
    "[map url / map id] [+mods] [-a acc%] [-c combo] [-300 #300s] [-100 #100s] [-50 #50s] [-m #misses] [-s score]"
)]
#[example(
    "1980365 +hddt -a 99.3 -c 1234 -300 1422 -50 2 -m 1",
    "https://osu.ppy.sh/beatmapsets/948199#osu/1980365 -a 97.56"
)]
#[aliases("s")]
async fn simulate(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = match SimulateMapArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => return msg.error(&ctx, err_msg).await,
    };

    let map_id_opt = args
        .map_id
        .or_else(|| {
            msg.referenced_message
                .as_ref()
                .filter(|_| msg.kind == MessageType::Reply)
                .and_then(|msg| map_id_from_msg(msg))
        })
        .or_else(|| {
            ctx.cache
                .message_extract(msg.channel_id, cached_message_extract)
        });

    let map_id = if let Some(id) = map_id_opt {
        id
    } else {
        let msgs = match ctx.retrieve_channel_history(msg.channel_id).await {
            Ok(msgs) => msgs,
            Err(why) => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                return Err(why.into());
            }
        };

        match map_id_from_history(&msgs) {
            Some(id) => id,
            None => {
                let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";

                return msg.error(&ctx, content).await;
            }
        }
    };

    let map_id = match map_id {
        MapIdType::Map(id) => id,
        MapIdType::Set(_) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return msg.error(&ctx, content).await;
        }
    };

    // Retrieving the beatmap
    let mut map = match ctx.psql().get_beatmap(map_id, true).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => map,
            Err(OsuError::NotFound) => {
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

    let mapset: BeatmapsetCompact = map.mapset.take().unwrap().into();

    // Accumulate all necessary data
    let data = match SimulateEmbed::new(None, &map, &mapset, args.into()).await {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .content("Simulated score:")?
        .embed(data.as_builder().build())?
        .await?;

    ctx.store_msg(response.id);
    response.reaction_delete(&ctx, msg.author.id);

    // Add map to database if its not in already
    if let Err(why) = ctx.psql().insert_beatmap(&map).await {
        unwind_error!(warn, why, "Could not add map to DB: {}");
    }

    // Set map on garbage collection list if unranked
    let gb = ctx.map_garbage_collector(&map);

    // Minimize embed after delay
    tokio::spawn(async move {
        gb.execute(&ctx).await;
        time::sleep(Duration::from_secs(45)).await;

        if !ctx.remove_msg(response.id) {
            return;
        }

        let _ = ctx
            .http
            .update_message(response.channel_id, response.id)
            .embed(data.into_builder().build())
            .unwrap()
            .await;
    });

    Ok(())
}
