use crate::{
    arguments::NameArgs,
    database::MySQL,
    embeds::RecentData,
    util::{
        discord,
        globals::{MINIMIZE_DELAY, OSU_API_ISSUE},
    },
    DiscordLinks, Osu, SchedulerKey,
};

use rosu::{
    backend::requests::RecentRequest,
    models::{
        ApprovalStatus::{Approved, Loved, Qualified, Ranked},
        GameMode,
    },
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use tokio::runtime::Runtime;
use white_rabbit::{DateResult, Duration, Utc};

fn recent_send(mode: GameMode, ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let args = NameArgs::new(args);
    let name = if let Some(name) = args.name {
        name
    } else {
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
    };
    let mut rt = Runtime::new().unwrap();

    // Retrieve the recent scores
    let scores = {
        let request = RecentRequest::with_username(&name).mode(mode).limit(50);
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        match rt.block_on(request.queue(osu)) {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };
    let score = if let Some(score) = scores.get(0) {
        score.clone()
    } else {
        msg.channel_id.say(
            &ctx.http,
            format!("No recent plays found for user `{}`", name),
        )?;
        return Ok(());
    };

    // Retrieving the score's user
    let user = {
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        match rt.block_on(score.get_user(osu, mode)) {
            Ok(u) => u,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };
    let map_id = score.beatmap_id.unwrap();

    // Retrieving the score's beatmap
    let (map_to_db, map) = {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        match mysql.get_beatmap(map_id) {
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

    // Retrieving the user's top 100 and the map's global top 50
    let (best, global) = {
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let best = match rt.block_on(user.get_top_scores(osu, 100, mode)) {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        let global = match map.approval_status {
            Ranked | Loved | Qualified | Approved => {
                match rt.block_on(map.get_global_leaderboard(osu, 100)) {
                    Ok(scores) => scores,
                    Err(why) => {
                        msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                        return Err(CommandError::from(why.to_string()));
                    }
                }
            }
            _ => Vec::new(),
        };
        (best, global)
    };

    // Accumulate all necessary data
    let map_copy = if map_to_db { Some(map.clone()) } else { None };
    let tries = scores
        .iter()
        .take_while(|s| s.beatmap_id.unwrap() == map_id)
        .count();
    let data = match RecentData::new(user, score, map, best, global, &ctx) {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id.say(
                &ctx.http,
                "Some issue while calculating recent data, blame bade",
            )?;
            return Err(CommandError::from(why.to_string()));
        }
    };

    // Creating the embed
    let mut response = msg.channel_id.send_message(&ctx.http, |m| {
        m.content(format!("Try #{}", tries))
            .embed(|e| data.build(e))
    })?;

    // Add map to database if its not in already
    if let Some(map) = map_copy {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.insert_beatmap(&map) {
            warn!("Could not add map of recent command to database: {}", why);
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
                warn!("Error while trying to minimize recent msg: {}", why);
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
#[description = "Display a user's most recent play"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("r", "rs")]
pub fn recent(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::STD, ctx, msg, args)
}

#[command]
#[description = "Display a user's most recent mania play"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("rm")]
pub fn recentmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::MNA, ctx, msg, args)
}

#[command]
#[description = "Display a user's most recent taiko play"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("rt")]
pub fn recenttaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::TKO, ctx, msg, args)
}

#[command]
#[description = "Display a user's most recent ctb play"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("rc")]
pub fn recentctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::CTB, ctx, msg, args)
}
