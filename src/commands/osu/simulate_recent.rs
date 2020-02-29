use crate::{
    database::MySQL,
    messages::SimulateData,
    util::{
        discord,
        globals::{MINIMIZE_DELAY, OSU_API_ISSUE},
    },
    DiscordLinks, Osu, SchedulerKey,
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
use std::error::Error;
use tokio::runtime::Runtime;
use white_rabbit::{DateResult, Duration, Utc};

fn simulate_recent_send(
    mode: GameMode,
    ctx: &mut Context,
    msg: &Message,
    mut args: Args,
) -> CommandResult {
    let name: String = if args.is_empty() {
        let data = ctx.data.read();
        let links = data
            .get::<DiscordLinks>()
            .expect("Could not get DiscordLinks");
        match links.get(msg.author.id.as_u64()) {
            Some(name) => name.clone(),
            None => {
                msg.channel_id.say(
                    &ctx.http,
                    "Either specify an osu name or link your discord \
                     to an osu profile via `<link osuname`",
                )?;
                return Ok(());
            }
        }
    } else {
        args.single_quoted()?
    };
    let mut rt = Runtime::new().unwrap();

    // Retrieve the recent score
    let score = {
        let request = RecentRequest::with_username(&name).mode(mode).limit(1);
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let mut scores = match rt.block_on(request.queue(osu)) {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        match scores.pop() {
            Some(score) => score,
            None => {
                msg.channel_id.say(
                    &ctx.http,
                    format!("No recent plays found for user `{}`", name),
                )?;
                return Ok(());
            }
        }
    };

    // Retrieving the score's beatmap
    let (map_to_db, map) = {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        match mysql.get_beatmap(score.beatmap_id.unwrap()) {
            Ok(map) => (false, map),
            Err(_) => {
                let osu = data.get::<Osu>().expect("Could not get osu client");
                let map = match rt.block_on(score.get_beatmap(osu)) {
                    Ok(m) => m,
                    Err(why) => {
                        msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
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
    let data = match SimulateData::new(Some(score), map, &ctx) {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id.say(
                &ctx.http,
                "Some issue while calculating simulaterecent data, blame bade",
            )?;
            return Err(CommandError::from(why.description()));
        }
    };

    // Creating the embed
    let mut response = msg.channel_id.send_message(&ctx.http, |m| {
        m.content("Simulated score:").embed(|e| data.build(e))
    })?;

    // Add map to database if its not in already
    if let Some(map) = map_copy {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.insert_beatmap(&map) {
            warn!(
                "Could not add map of simulaterecent command to database: {}",
                why
            );
        }
    }

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());

    // Minimize embed after delay
    let scheduler = {
        let mut data = ctx.data.write();
        data.get_mut::<SchedulerKey>()
            .expect("Could not get SchedulerKey")
            .clone()
    };
    let mut scheduler = scheduler.write();
    let http = ctx.http.clone();
    let cache = ctx.cache.clone();
    let mut retries = 5;
    scheduler.add_task_duration(Duration::seconds(MINIMIZE_DELAY), move |_| {
        if let Err(why) = response.edit((&cache, &*http), |m| m.embed(|e| data.minimize(e))) {
            if retries == 0 {
                warn!(
                    "Error while trying to minimize simulate recent msg: {}",
                    why
                );
                DateResult::Done
            } else {
                retries -= 1;
                DateResult::Repeat(Utc::now() + Duration::seconds(5))
            }
        } else {
            DateResult::Done
        }
    });
    Ok(())
}

#[command]
#[description = "Display an unchoked version of user's most recent play"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("sr")]
pub fn simulaterecent(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    simulate_recent_send(GameMode::STD, ctx, msg, args)
}

#[command]
#[description = "Display a perfect play on a user's most recently played mania map"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("srm")]
pub fn simulaterecentmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    simulate_recent_send(GameMode::MNA, ctx, msg, args)
}
