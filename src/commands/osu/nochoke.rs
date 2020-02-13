use crate::{
    database::MySQL,
    messages::{BotEmbed, NoChokeData},
    util::{globals::OSU_API_ISSUE, osu},
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::{OsuArgs, OsuRequest, UserArgs, UserBestArgs},
    models::{Beatmap, GameMode, Score, User},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::{misc::Mentionable, prelude::Message},
    prelude::Context,
};
use std::{collections::HashMap, error::Error as StdError};
use tokio::runtime::Runtime;

#[command]
#[description = "Display a user's top plays if no score in their top 100 would be a choke"]
#[usage = "badewanne3"]
#[aliases("nc", "nochokes")]
fn nochoke(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
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
    let user_args = UserArgs::with_username(&name);
    let best_args = UserBestArgs::with_username(&name).limit(100);
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

    let mention = msg.author.mention();
    let mut msg_content = format!("Gathering data for `{}`, I'll ping you when I'm done", name);
    let mut msg = msg
        .channel_id
        .send_message(&ctx.http, |m| m.content(&msg_content))?;

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
    if maps.len() != scores.len() {
        msg_content.push_str(&format!(
            "\nRetrieving {} maps from the API...",
            scores.len() - maps.len()
        ));
        msg.edit(&ctx, |m| m.content(&msg_content))?;
    }

    // Retrieving all missing beatmaps
    let res = rt.block_on(async move {
        let mut data = HashMap::with_capacity(scores.len());
        let mut missing_indices = Vec::with_capacity(scores.len());
        for (i, score) in scores.into_iter().enumerate() {
            let map_id = score.beatmap_id.unwrap();
            let map = if maps.contains_key(&map_id) {
                maps.remove(&map_id).unwrap()
            } else {
                missing_indices.push(i + 1);
                score
                    .beatmap
                    .as_ref()
                    .unwrap()
                    .get(GameMode::STD)
                    .await
                    .or_else(|e| {
                        Err(CommandError(format!(
                            "Error while retrieving LazilyLoaded<Beatmap> of best score: {}",
                            e
                        )))
                    })?
            };
            data.insert(i + 1, (score, map));
        }
        Ok((data, missing_indices))
    });
    let (scores_data, missing_maps): (HashMap<usize, _>, Option<Vec<Beatmap>>) = match res {
        Ok((score_maps, missing_indices)) => {
            let missing_maps = if missing_indices.is_empty() {
                None
            } else {
                Some(
                    score_maps
                        .iter()
                        .filter(|(i, ..)| missing_indices.contains(i))
                        .map(|(_, (_, map))| map.clone())
                        .collect(),
                )
            };
            for (_, (_, map)) in score_maps.iter() {
                if let Err(why) = osu::prepare_beatmap_file(map.beatmap_id) {
                    msg.edit(&ctx, |m| {
                        m.content("Something went wrong while downloading a beatmap, blame bade")
                    })?;
                    return Err(CommandError::from(why.description()));
                }
            }
            (score_maps, missing_maps)
        }
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
        }
    };

    msg_content.push_str("\nAll data prepared, now calculating...");
    msg.edit(&ctx, |m| m.content(msg_content))?;

    // Accumulate all necessary data
    let data = NoChokeData::create(user, scores_data, ctx.cache.clone());

    // Creating the embed
    let embed = BotEmbed::UserMapMulti(data);
    let msg_res = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.content(format!("{} No-choke top scores for `{}`:", mention, name))
                .embed(|e| embed.create(e))
        })
        .and(msg.delete(&ctx));

    // Add missing maps to database
    if let Some(maps) = missing_maps {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.insert_beatmaps(maps) {
            warn!(
                "Could not add missing maps of nochoke command to database: {}",
                why
            );
        }
    }
    msg_res?;
    Ok(())
}
