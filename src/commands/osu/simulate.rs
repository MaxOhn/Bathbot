use crate::{
    arguments::{Args, SimulateMapArgs},
    embeds::{EmbedData, SimulateEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        discord, MessageExt,
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
async fn simulate(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let args = match SimulateMapArgs::new(Args::new(msg.content.clone())) {
        Ok(args) => args,
        Err(err_msg) => {
            msg.respond(&ctx, err_msg).await?;
            return Ok(());
        }
    };
    let map_id = if let Some(map_id) = args.map_id {
        map_id
    } else {
        let msgs = msg
            .channel_id
            .messages(ctx, |retriever| retriever.limit(50))
            .await?;
        match discord::map_id_from_history(msgs, &ctx.cache).await {
            Some(id) => id,
            None => {
                let content = "No map embed found in this channel's recent history.\n\
                        Try specifying a map either by url to the map, \
                        or just by map id.";
                msg.respond(&ctx, content).await?;
                return Ok(());
            }
        }
    };

    // Retrieving the beatmap
    let (map_to_db, map) = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        match mysql.get_beatmap(map_id).await {
            Ok(map) => (false, map),
            Err(_) => {
                let map_req = BeatmapRequest::new().map_id(map_id);
                let osu = data.get::<Osu>().unwrap();
                let map = match map_req.queue_single(&osu).await {
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
                };
                (
                    map.approval_status == Ranked
                        || map.approval_status == Loved
                        || map.approval_status == Approved,
                    map,
                )
            }
        }
    };

    match map.mode {
        GameMode::TKO | GameMode::CTB => {
            let content = format!("I can only simulate STD and MNA maps, not {}", map.mode);
            msg.respond(&ctx, content).await?;
            return Ok(());
        }
        _ => {}
    }

    // Accumulate all necessary data
    let map_copy = if map_to_db { Some(map.clone()) } else { None };
    let data = match SimulateEmbed::new(None, map, args.into(), ctx).await {
        Ok(data) => data,
        Err(why) => {
            msg.respond(&ctx, GENERAL_ISSUE).await?;
            return Err(why);
        }
    };

    // Creating the embed
    let mut response = msg
        .channel_id
        .send_message(ctx, |m| m.embed(|e| data.build(e)))
        .await?;

    // Add map to database if its not in already
    if let Some(map) = map_copy {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        if let Err(why) = mysql.insert_beatmap(&map).await {
            warn!("Could not add map of simulaterecent command to DB: {}", why);
        }
    }
    response.clone().reaction_delete(ctx, msg.author.id).await;

    // Minimize embed after delay
    time::delay_for(Duration::from_secs(45)).await;
    if let Err(why) = response.edit(ctx, |m| m.embed(|e| data.minimize(e))).await {
        warn!("Error while minimizing simulate msg: {}", why);
    }
    Ok(())
}
