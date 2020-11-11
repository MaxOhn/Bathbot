use crate::{
    arguments::{Args, SimulateNameArgs},
    embeds::{EmbedData, SimulateEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, Context,
};

use rosu::model::GameMode;
use std::sync::Arc;
use tokio::time::{self, Duration};
use twilight_model::channel::Message;

#[allow(clippy::cognitive_complexity)]
async fn simulate_recent_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let mut args = match SimulateNameArgs::new(&ctx, args) {
        Ok(args) => args,
        Err(err_msg) => return msg.error(&ctx, err_msg).await,
    };
    let name = match args.name.take().or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    // Retrieve the recent score
    let scores_fut = ctx.osu().recent_scores(name.as_str()).mode(mode).limit(1);
    let score = match scores_fut.await {
        Ok(mut scores) => match scores.pop() {
            Some(score) => score,
            None => {
                let content = format!(
                    "No recent {}plays found for user `{}`",
                    match mode {
                        GameMode::STD => "",
                        GameMode::TKO => "taiko ",
                        GameMode::CTB => "ctb ",
                        GameMode::MNA => "mania ",
                    },
                    name
                );
                return msg.error(&ctx, content).await;
            }
        },
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };

    // Retrieving the score's beatmap
    let map_id = score.beatmap_id.unwrap();
    let map = match ctx.psql().get_beatmap(map_id).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(Some(m)) => m,
            Ok(None) => {
                let content = format!("The API returned no beatmap for map id {}", map_id);
                return msg.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;
                return Err(why.into());
            }
        },
    };

    // Accumulate all necessary data
    let data = match SimulateEmbed::new(&ctx, Some(score), &map, args.into()).await {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            return Err(why);
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
        if ctx.remove_msg(response.id) {
            let embed = data.minimize().build().unwrap();
            let _ = ctx
                .http
                .update_message(response.channel_id, response.id)
                .embed(embed)
                .unwrap()
                .await;
        }
    });
    Ok(())
}

#[command]
#[short_desc("Unchoke a user's most recent play")]
#[usage(
    "[username] [+mods] [-a acc%] [-c combo] [-300 #300s] [-100 #100s] [-50 #50s] [-m #misses]"
)]
#[example("badewanne3 +hr -a 99.3 -300 1422 -m 1")]
#[aliases("sr")]
pub async fn simulaterecent(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    simulate_recent_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Display a perfect play on a user's most recently played map")]
#[usage("[username] [+mods] [-s score]")]
#[example("badewanne3 +dt -s 895000")]
#[aliases("srm")]
pub async fn simulaterecentmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    simulate_recent_main(GameMode::MNA, ctx, msg, args).await
}
