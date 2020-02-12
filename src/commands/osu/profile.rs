use crate::{
    database::MySQL,
    messages::{BotEmbed, ProfileData},
    util::globals::OSU_API_ISSUE,
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::{OsuArgs, OsuRequest, UserArgs, UserBestArgs},
    models::{Beatmap, GameMode, Score, User},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::collections::HashMap;
use tokio::runtime::Runtime;

fn profile_send(mode: GameMode, ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
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
    let user_args = UserArgs::with_username(&name).mode(mode);
    let best_args = UserBestArgs::with_username(&name).mode(mode).limit(100);
    let (user_req, best_req): (OsuRequest<User>, OsuRequest<Score>) = {
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let user_req = osu.create_request(OsuArgs::Users(user_args));
        let best_req = osu.create_request(OsuArgs::Best(best_args));
        (user_req, best_req)
    };
    let mut rt = Runtime::new().unwrap();

    // Retrieve the user and its top scores
    let res = rt.block_on(async {
        let users = user_req
            .queue()
            .await
            .or_else(|e| Err(CommandError(format!("Error while retrieving Users: {}", e))))?;
        let scores = best_req.queue().await.or_else(|e| {
            Err(CommandError(format!(
                "Error while retrieving UserBest: {}",
                e
            )))
        })?;
        Ok((users, scores))
    });
    let (user, scores): (User, Vec<Score>) = match res {
        Ok((mut users, scores)) => {
            let user = match users.pop() {
                Some(user) => user,
                None => {
                    msg.channel_id
                        .say(&ctx.http, format!("User `{}` was not found", name))?;
                    return Ok(());
                }
            };
            (user, scores)
        }
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
        }
    };

    // Get all relevant maps from the database
    let map_ids: Vec<u32> = scores.iter().map(|s| s.beatmap_id.unwrap()).collect();
    let mut maps = {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql
            .get_beatmaps(&map_ids)
            .unwrap_or_else(|_| HashMap::default())
    };
    info!(
        "Found {}/{} beatmaps in the database",
        maps.len(),
        scores.len()
    );

    // Retrieving each score's beatmap
    let res = rt.block_on(async move {
        let mut tuples = Vec::with_capacity(scores.len());
        let mut missing_indices = Vec::with_capacity(scores.len());
        for (i, score) in scores.into_iter().enumerate() {
            let map_id = score.beatmap_id.unwrap();
            let map = if maps.contains_key(&map_id) {
                maps.remove(&map_id).unwrap()
            } else {
                missing_indices.push(i);
                score
                    .beatmap
                    .as_ref()
                    .unwrap()
                    .get(mode)
                    .await
                    .or_else(|e| {
                        Err(CommandError(format!(
                            "Error while retrieving LazilyLoaded<Beatmap> of best score: {}",
                            e
                        )))
                    })?
            };
            tuples.push((score, map));
        }
        Ok((tuples, missing_indices))
    });
    let (score_maps, missing_maps): (Vec<_>, Option<Vec<Beatmap>>) = match res {
        Ok((score_maps, missing_indices)) => {
            let missing_maps = if missing_indices.is_empty() {
                None
            } else {
                Some(
                    score_maps
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| missing_indices.contains(i))
                        .map(|(_, (_, map))| map.clone())
                        .collect(),
                )
            };
            (score_maps, missing_maps)
        }
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
        }
    };
    // Accumulate all necessary data
    let data = ProfileData::new(user, score_maps, mode, ctx.cache.clone());

    // Creating the embed
    let embed = BotEmbed::Profile(data);
    let _ = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| embed.create(e)));

    // Add missing maps to database
    if let Some(maps) = missing_maps {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.insert_beatmaps(maps) {
            warn!(
                "Could not add missing maps of profile command to database: {}",
                why
            );
        }
    }
    Ok(())
}

#[command]
#[description = "Display statistics of a user"]
#[usage = "badewanne3"]
#[aliases("osu")]
pub fn profile(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    profile_send(GameMode::STD, ctx, msg, args)
}

#[command]
#[description = "Display statistics of a mania user"]
#[usage = "badewanne3"]
#[aliases("mania", "maniaprofile")]
pub fn profilemania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    profile_send(GameMode::MNA, ctx, msg, args)
}

#[command]
#[description = "Display statistics of a taiko user"]
#[usage = "badewanne3"]
#[aliases("taiko", "taikoprofile")]
pub fn profiletaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    profile_send(GameMode::TKO, ctx, msg, args)
}

#[command]
#[description = "Display statistics of ctb user"]
#[usage = "badewanne3"]
#[aliases("ctb", "ctbprofile")]
pub fn profilectb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    profile_send(GameMode::CTB, ctx, msg, args)
}
