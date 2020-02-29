use crate::{
    commands::arguments,
    database::MySQL,
    messages::SimulateData,
    util::{
        discord,
        globals::{MINIMIZE_DELAY, OSU_API_ISSUE},
    },
    Osu, SchedulerKey,
};

use rosu::{
    backend::requests::BeatmapRequest,
    models::ApprovalStatus::{Loved, Ranked},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use tokio::runtime::Runtime;
use white_rabbit::{DateResult, Duration, Utc};

#[command]
#[description = "Simulate a (perfect) score on the given map. \
                 If no map is given, I will choose the last map \
                 I can find in my embeds of this channel"]
#[usage = "[map url / map id]"]
#[example = "1980365"]
#[example = "https://osu.ppy.sh/beatmapsets/948199#osu/1980365"]
#[aliases("s")]
fn simulate(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse the beatmap id
    let map_id = match args.len() {
        0 => {
            msg.channel_id.say(
                &ctx.http,
                "You need to provide a beatmap, either as map id or as url",
            )?;
            return Ok(());
        }
        _ => {
            if let Some(map_id) = arguments::get_regex_id(&args.single::<String>()?) {
                map_id
            } else {
                msg.channel_id.say(
                    &ctx.http,
                    "If no osu name is provided, the first argument must be a beatmap id.\n\
                     If you want to give an osu name, do so as first argument.\n\
                     The second argument should then be the beatmap id.\n\
                     The beatmap id can be given as number or as URL to the beatmap.",
                )?;
                return Ok(());
            }
        }
    };

    // Retrieving the beatmap
    let (map_to_db, map) = {
        let mut rt = Runtime::new().expect("Could not create runtime");
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        match mysql.get_beatmap(map_id) {
            Ok(map) => (false, map),
            Err(_) => {
                let map_req = BeatmapRequest::new().map_id(map_id);
                let osu = data.get::<Osu>().expect("Could not get osu client");
                let map = match rt.block_on(map_req.queue_single(&osu)) {
                    Ok(result) => match result {
                        Some(map) => map,
                        None => {
                            msg.channel_id.say(
                                &ctx.http,
                                format!("Could not find beatmap with id `{}`. Did you give me a mapset id instead of a map id?", map_id),
                            )?;
                            return Ok(());
                        }
                    },
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
    let data = match SimulateData::new(None, map, &ctx) {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id.say(
                &ctx.http,
                "Some issue while calculating scores data, blame bade",
            )?;
            return Err(CommandError::from(why.to_string()));
        }
    };

    // Creating the embed
    let mut response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))?;

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
                warn!("Error while trying to minimize simulate msg: {}", why);
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
