use crate::{
    commands::osu::MINIMIZE_DELAY,
    database::MySQL,
    messages::{BotEmbed, ScoreSingleData},
    util::globals::OSU_API_ISSUE,
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::{OsuArgs, OsuRequest, ScoreArgs, UserBestArgs, UserRecentArgs},
    models::{
        ApprovalStatus::{Approved, Loved, Qualified, Ranked},
        GameMode, Grade, Score,
    },
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::thread;
use tokio::runtime::Runtime;

fn recent_send(mode: GameMode, ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
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
    let recent_args = UserRecentArgs::with_username(&name).mode(mode).limit(50);
    let recent_req: OsuRequest<Score> = {
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        osu.create_request(OsuArgs::Recent(recent_args))
    };
    let mut rt = Runtime::new().unwrap();

    // Retrieve the recent scores
    let scores: Vec<Score> = match rt.block_on(recent_req.queue()) {
        Ok(scores) => scores,
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(CommandError(format!(
                "Error while retrieving UserRecent: {}",
                why
            )));
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
    let user = match rt.block_on(score.user.get(mode)) {
        Ok(u) => u,
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(CommandError(format!(
                "Error while retrieving LazilyLoaded<User> of recent: {}",
                why
            )));
        }
    };
    let map_id = score.beatmap_id.unwrap();

    // Retrieving the score's beatmap
    let map = {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql.get_beatmap(map_id)
    };
    let (map_to_db, map) = if let Ok(map) = map {
        (false, map)
    } else {
        let map = match rt.block_on(score.beatmap.as_ref().unwrap().get(mode)) {
            Ok(m) => m,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError(format!(
                    "Error while retrieving LazilyLoaded<Beatmap> of recent: {}",
                    why
                )));
            }
        };
        (
            map.approval_status == Ranked || map.approval_status == Loved,
            map,
        )
    };

    // Retrieving the user's top 100 and the map's global top 50
    let best_args = UserBestArgs::with_username(&name).mode(mode).limit(100);
    let global_args = ScoreArgs::with_map_id(map_id).mode(mode).limit(50);
    let (best_req, global_req) = {
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let best_req = osu.create_request(OsuArgs::Best(best_args));
        let global_req = osu.create_request(OsuArgs::Scores(global_args));
        (best_req, global_req)
    };
    let res = rt.block_on(async {
        if score.grade == Grade::F {
            return Ok((Vec::new(), Vec::new()));
        }
        let best = if map.approval_status == Ranked {
            best_req.queue().await.or_else(|e| {
                Err(CommandError(format!(
                    "Error while retrieving UserBest: {}",
                    e
                )))
            })?
        } else {
            Vec::new()
        };
        let global = match map.approval_status {
            Ranked | Loved | Qualified | Approved => global_req.queue().await.or_else(|e| {
                Err(CommandError(format!(
                    "Error while retrieving Scores: {}",
                    e
                )))
            })?,
            _ => Vec::new(),
        };
        Ok((best, global))
    });
    let (best, global) = match res {
        Ok(tuple) => tuple,
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
        }
    };

    // Accumulate all necessary data
    let map_copy = if map_to_db { Some(map.clone()) } else { None };
    let tries = scores
        .iter()
        .take_while(|s| s.beatmap_id.unwrap() == map_id)
        .count();
    let data = ScoreSingleData::new(user, score, map, best, global, mode, ctx.cache.clone());

    // Creating the embed
    let embed = BotEmbed::UserScoreSingle(&data);
    let mut msg = msg.channel_id.send_message(&ctx.http, |m| {
        m.content(format!("Try #{}", tries))
            .embed(|e| embed.create(e))
    })?;

    // Add map to database if its not in already
    if let Some(map) = map_copy {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.insert_beatmap(&map) {
            warn!("Could not add map of recent command to database: {}", why);
        }
    }

    // Minimize embed after delay
    let embed = BotEmbed::UserScoreSingleMini(Box::new(data));
    msg.edit(&ctx, |m| {
        thread::sleep(MINIMIZE_DELAY);
        m.embed(|e| embed.create(e))
    })?;
    Ok(())
}

#[command]
#[description = "Display a user's most recent play"]
#[usage = "badewanne3"]
#[aliases("r", "rs")]
pub fn recent(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::STD, ctx, msg, args)
}

#[command]
#[description = "Display a user's most recent mania play"]
#[usage = "badewanne3"]
#[aliases("rm")]
pub fn recentmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::MNA, ctx, msg, args)
}

#[command]
#[description = "Display a user's most recent taiko play"]
#[usage = "badewanne3"]
#[aliases("rt")]
pub fn recenttaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::TKO, ctx, msg, args)
}

#[command]
#[description = "Display a user's most recent ctb play"]
#[usage = "badewanne3"]
#[aliases("rc")]
pub fn recentctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::CTB, ctx, msg, args)
}
