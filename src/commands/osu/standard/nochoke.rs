use crate::{
    database::MySQL,
    messages::BasicEmbedData,
    util::{discord, globals::OSU_API_ISSUE},
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::UserRequest,
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
#[description = "Display a user's top plays if no score in their top 100 \
                 would be a choke"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("nc", "nochoke")]
fn nochokes(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
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

    // Retrieve the user and its top scores
    let (user, scores): (User, Vec<Score>) = {
        let user_req = UserRequest::with_username(&name).mode(GameMode::STD);
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
        let scores = match rt.block_on(user.get_top_scores(&osu, 100, GameMode::STD)) {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        (user, scores)
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

    // Further prepare data and retrieve missing maps
    let (scores_data, missing_maps): (HashMap<usize, _>, Option<Vec<Beatmap>>) = {
        let mut scores_data = HashMap::with_capacity(scores.len());
        let mut missing_maps = Vec::new();
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        for (i, score) in scores.into_iter().enumerate() {
            let map_id = score.beatmap_id.unwrap();
            let map = if maps.contains_key(&map_id) {
                maps.remove(&map_id).unwrap()
            } else {
                let map = match rt.block_on(score.get_beatmap(osu)) {
                    Ok(map) => map,
                    Err(why) => {
                        msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                        return Err(CommandError::from(why.to_string()));
                    }
                };
                missing_maps.push(map.clone());
                map
            };
            scores_data.insert(i + 1, (score, map));
        }
        (
            scores_data,
            if missing_maps.is_empty() {
                None
            } else {
                Some(missing_maps)
            },
        )
    };
    msg_content.push_str("\nAll data prepared, now calculating...");
    msg.edit(&ctx, |m| m.content(msg_content))?;

    // Accumulate all necessary data
    let data = match BasicEmbedData::create_nochoke(user, scores_data, ctx.cache.clone()) {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id.say(
                &ctx.http,
                "Some issue while calculating nochoke data, blame bade",
            )?;
            return Err(CommandError::from(why.description()));
        }
    };

    // Creating the embed
    let response = msg.channel_id.send_message(&ctx.http, |m| {
        m.content(format!("{} No-choke top scores for `{}`:", mention, name))
            .embed(|e| data.build(e))
    });
    let _ = msg.delete(&ctx);

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

    // Save the response owner
    discord::save_response_owner(response?.id, msg.author.id, ctx.data.clone());
    Ok(())
}
