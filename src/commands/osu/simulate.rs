use crate::{
    arguments::{Args, SimulateMapArgs},
    bail,
    embeds::{EmbedData, SimulateEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        osu::{cached_message_extract, map_id_from_history, MapIdType},
        MessageExt,
    },
    BotResult, Context,
};

use rosu::model::GameMode;
use std::sync::Arc;
use tokio::time::{self, Duration};
use twilight_model::channel::Message;

#[command]
#[short_desc("Simulate a score on a map")]
#[long_desc(
    "Simulate a (perfect) score on the given map. \
     Mods can be specified.\n\
     If no map is given, I will choose the last map \
     I can find in my embeds of this channel.\n\
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
    let map_id = if let Some(id) = args.map_id {
        id
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
                bail!("error while retrieving messages: {}", why);
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

    if let GameMode::TKO | GameMode::CTB = map.mode {
        let content = format!("I can only simulate STD and MNA maps, not {}", map.mode);
        return msg.error(&ctx, content).await;
    }

    // Accumulate all necessary data
    let data = match SimulateEmbed::new(&ctx, None, &map, args.into()).await {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            bail!("error while creating embed: {}", why);
        }
    };

    // Creating the embed
    let embed = data.build().build()?;
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .content("Simulated score:")?
        .embed(embed)?
        .await?;
    ctx.store_msg(response.id);

    // Add map to database if its not in already
    if let Err(why) = ctx.psql().insert_beatmap(&map).await {
        warn!("Could not add map to DB: {}", why);
    }
    response.reaction_delete(&ctx, msg.author.id);

    // Minimize embed after delay
    tokio::spawn(async move {
        time::delay_for(Duration::from_secs(45)).await;
        if !ctx.remove_msg(response.id) {
            return;
        }
        let embed = data.minimize().build().unwrap();
        let _ = ctx
            .http
            .update_message(response.channel_id, response.id)
            .embed(embed)
            .unwrap()
            .await;
    });
    Ok(())
}
