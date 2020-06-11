use crate::{
    arguments::SimulateNameArgs,
    database::MySQL,
    embeds::SimulateData,
    util::{
        globals::{MINIMIZE_DELAY, OSU_API_ISSUE},
        MessageExt,
    },
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::RecentRequest,
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

#[allow(clippy::cognitive_complexity)]
async fn simulate_recent_send(
    mode: GameMode,
    ctx: &Context,
    msg: &Message,
    args: Args,
) -> CommandResult {
    let args = match SimulateNameArgs::new(args) {
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
    let name = if let Some(name) = args.name.as_ref() {
        name.clone()
    } else {
        let data = ctx.data.read().await;
        let links = data.get::<DiscordLinks>().unwrap();
        match links.get(msg.author.id.as_u64()) {
            Some(name) => name.clone(),
            None => {
                msg.channel_id
                    .say(
                        ctx,
                        "Either specify an osu name or link your discord \
                        to an osu profile via `<link osuname`",
                    )
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Ok(());
            }
        }
    };

    // Retrieve the recent score
    let score = {
        let request = RecentRequest::with_username(&name).mode(mode).limit(1);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        let mut scores = match request.queue(osu).await {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id
                    .say(&ctx.http, OSU_API_ISSUE)
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(CommandError::from(why.to_string()));
            }
        };
        match scores.pop() {
            Some(score) => score,
            None => {
                msg.channel_id
                    .say(ctx, format!("No recent plays found for user `{}`", name))
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Ok(());
            }
        }
    };

    // Retrieving the score's beatmap
    let (map_to_db, map) = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        match mysql.get_beatmap(score.beatmap_id.unwrap()) {
            Ok(map) => (false, map),
            Err(_) => {
                let osu = data.get::<Osu>().unwrap();
                let map = match score.get_beatmap(osu).await {
                    Ok(m) => m,
                    Err(why) => {
                        msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
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

    // Accumulate all necessary data
    let map_copy = if map_to_db { Some(map.clone()) } else { None };
    let data = match SimulateData::new(Some(score), map, args.into(), ctx).await {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id
                .say(
                    ctx,
                    "Some issue while calculating simulaterecent data, blame bade",
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
        .send_message(&ctx.http, |m| {
            m.content("Simulated score:").embed(|e| data.build(e))
        })
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
    time::delay_for(Duration::from_secs(MINIMIZE_DELAY)).await;
    if let Err(why) = response.edit(ctx, |m| m.embed(|e| data.minimize(e))).await {
        warn!(
            "Error while trying to minimize simulate recent msg: {}",
            why
        );
    }
    Ok(())
}

#[command]
#[description = "Display an unchoked version of user's most recent play"]
#[usage = "[username] [-a acc%] [-300 #300s] [-100 #100s] [-50 #50s] [-m #misses]"]
#[example = "badewanne3 -a 99.3 -300 1422 -m 1"]
#[aliases("sr")]
pub async fn simulaterecent(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    simulate_recent_send(GameMode::STD, ctx, msg, args).await
}

#[command]
#[description = "Display a perfect play on a user's most recently played mania map"]
#[usage = "[username] [-s score]"]
#[example = "badewanne3 -s 8950000"]
#[aliases("srm")]
pub async fn simulaterecentmania(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    simulate_recent_send(GameMode::MNA, ctx, msg, args).await
}
