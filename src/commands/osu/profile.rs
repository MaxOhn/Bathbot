use crate::{
    arguments::NameArgs,
    database::MySQL,
    embeds::BasicEmbedData,
    util::{discord, globals::OSU_API_ISSUE},
    DiscordLinks, Osu,
};

use rayon::prelude::*;
use rosu::{
    backend::requests::UserRequest,
    models::{Beatmap, GameMode, Score, User},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::collections::HashMap;
use tokio::runtime::Runtime;

fn profile_send(mode: GameMode, ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
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

    // Retrieve the user and its top scores
    let (user, scores): (User, Vec<Score>) = {
        let user_req = UserRequest::with_username(&name).mode(mode);
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let user = match rt.block_on(user_req.queue_single(&osu)) {
            Ok(result) => match result {
                Some(user) => user,
                None => {
                    msg.channel_id
                        .say(&ctx.http, format!("User `{}` was not found", name))?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        let scores = match rt.block_on(user.get_top_scores(&osu, 100, mode)) {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        (user, scores)
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

    // Retrieving all missing beatmaps
    let res = rt.block_on(async {
        let mut tuples = Vec::with_capacity(scores.len());
        let mut missing_indices = Vec::with_capacity(scores.len());
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        for (i, score) in scores.into_iter().enumerate() {
            let map_id = score.beatmap_id.unwrap();
            let map = if maps.contains_key(&map_id) {
                maps.remove(&map_id).unwrap()
            } else {
                missing_indices.push(i);
                score.get_beatmap(osu).await.or_else(|e| {
                    Err(CommandError(format!(
                        "Error while retrieving Beatmap of score: {}",
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
                        .par_iter()
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
    let data = BasicEmbedData::create_profile(user, score_maps, mode, ctx.cache.clone());

    // Send the embed
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)));

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

    // Save the response owner
    discord::save_response_owner(response?.id, msg.author.id, ctx.data.clone());
    Ok(())
}

#[command]
#[description = "Display statistics of a user"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("osu")]
pub fn profile(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    profile_send(GameMode::STD, ctx, msg, args)
}

#[command]
#[description = "Display statistics of a mania user"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("mania", "maniaprofile")]
pub fn profilemania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    profile_send(GameMode::MNA, ctx, msg, args)
}

#[command]
#[description = "Display statistics of a taiko user"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("taiko", "taikoprofile")]
pub fn profiletaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    profile_send(GameMode::TKO, ctx, msg, args)
}

#[command]
#[description = "Display statistics of ctb user"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("ctb", "ctbprofile")]
pub fn profilectb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    profile_send(GameMode::CTB, ctx, msg, args)
}
