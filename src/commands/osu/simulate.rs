use crate::{
    arguments::SimulateMapArgs,
    database::MySQL,
    embeds::{EmbedData, SimulateEmbed},
    util::{discord, globals::OSU_API_ISSUE, MessageExt},
    Osu,
};

use rosu::{
    backend::requests::BeatmapRequest,
    models::{
        ApprovalStatus::{Loved, Ranked},
        GameMode,
    },
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use tokio::time::{self, Duration};

#[command]
#[description = "Simulate a (perfect) score on the given map. \
                 If no map is given, I will choose the last map \
                 I can find in my embeds of this channel.\n\
                 The `-s` argument is only relevant for mania."]
#[usage = "[map url / map id] [-a acc%] [-300 #300s] [-100 #100s] [-50 #50s] [-m #misses] [-s score]"]
#[example = "1980365 -a 99.3 -300 1422 -m 1"]
#[example = "https://osu.ppy.sh/beatmapsets/948199#osu/1980365"]
#[aliases("s")]
async fn simulate(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let args = match SimulateMapArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => {
            msg.channel_id
                .say(ctx, err_msg)
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
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
                msg.channel_id
                    .say(
                        ctx,
                        "No map embed found in this channel's recent history.\n\
                        Try specifying a map either by url to the map, \
                        or just by map id.",
                    )
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Ok(());
            }
        }
    };

    // Retrieving the beatmap
    let (map_to_db, map) = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        match mysql.get_beatmap(map_id) {
            Ok(map) => (false, map),
            Err(_) => {
                let map_req = BeatmapRequest::new().map_id(map_id);
                let osu = data.get::<Osu>().unwrap();
                let map = match map_req.queue_single(&osu).await {
                    Ok(result) => match result {
                        Some(map) => map,
                        None => {
                            msg.channel_id
                                .say(
                                    ctx,
                                    format!(
                                        "Could not find beatmap with id `{}`. \
                                        Did you give me a mapset id instead of a map id?",
                                        map_id
                                    ),
                                )
                                .await?
                                .reaction_delete(ctx, msg.author.id)
                                .await;
                            return Ok(());
                        }
                    },
                    Err(why) => {
                        msg.channel_id.say(ctx, OSU_API_ISSUE).await?;
                        return Err(CommandError::from(why.to_string()));
                    }
                };
                (
                    map.approval_status == Ranked || map.approval_status == Loved,
                    map,
                )
            }
        }
    };

    match map.mode {
        GameMode::TKO | GameMode::CTB => {
            msg.channel_id
                .say(
                    ctx,
                    format!("I can only simulate STD and MNA maps, not {}", map.mode),
                )
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Ok(());
        }
        _ => {}
    }

    // Accumulate all necessary data
    let map_copy = if map_to_db { Some(map.clone()) } else { None };
    let data = match SimulateEmbed::new(None, map, args.into(), ctx).await {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id
                .say(
                    ctx,
                    "Some issue while calculating simulate data, blame bade",
                )
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Err(CommandError::from(why.to_string()));
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
        if let Err(why) = mysql.insert_beatmap(&map) {
            warn!("Could not add map of simulaterecent command to DB: {}", why);
        }
    }
    response.clone().reaction_delete(ctx, msg.author.id).await;

    // Minimize embed after delay
    for _ in 0..5usize {
        time::delay_for(Duration::from_secs(45)).await;
        match response.edit(ctx, |m| m.embed(|e| data.minimize(e))).await {
            Ok(_) => break,
            Err(why) => {
                warn!("Error while minimizing simulate msg: {}", why);
                time::delay_for(Duration::from_secs(5)).await;
            }
        }
    }
    Ok(())
}
