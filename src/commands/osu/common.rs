use crate::{
    arguments::MultNameArgs,
    database::MySQL,
    embeds::BasicEmbedData,
    util::{discord, globals::OSU_API_ISSUE},
    DiscordLinks, Osu,
};

use itertools::Itertools;
use rayon::prelude::*;
use rosu::{
    backend::requests::{BeatmapRequest, UserRequest},
    models::{Beatmap, GameMode, Score, User},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::{
    collections::{HashMap, HashSet},
    convert::From,
    fmt::Write,
};

#[allow(clippy::cognitive_complexity)]
async fn common_send(mode: GameMode, ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mut args = MultNameArgs::new(args, 10);
    let names = match args.names.len() {
        0 => {
            msg.channel_id
                .say(
                    &ctx.http,
                    "You need to specify at least one osu username. \
                 If you're not linked, you must specify at least two names.",
                )
                .await?;
            return Ok(());
        }
        1 => {
            let data = ctx.data.read().await;
            let links = data
                .get::<DiscordLinks>()
                .expect("Could not get DiscordLinks");
            match links.get(msg.author.id.as_u64()) {
                Some(name) => {
                    args.names.insert(name.clone());
                }
                None => {
                    msg.channel_id
                        .say(
                            &ctx.http,
                            "Since you're not linked via `<link`, \
                         you must specify at least two names.",
                        )
                        .await?;
                    return Ok(());
                }
            }
            args.names
        }
        _ => args.names,
    };
    if names.iter().collect::<HashSet<_>>().len() == 1 {
        msg.channel_id
            .say(&ctx.http, "Give at least two different names.")
            .await?;
        return Ok(());
    }

    // Retrieve all users and their top scores
    let requests: HashMap<String, UserRequest> = names
        .iter()
        .map(|name| (name.clone(), UserRequest::with_username(name).mode(mode)))
        .collect();
    let (users, mut all_scores): (HashMap<u32, User>, Vec<Vec<Score>>) = {
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let mut users = HashMap::with_capacity(requests.len());
        let mut all_scores = Vec::with_capacity(requests.len());
        for (name, request) in requests.into_iter() {
            let user = match request.queue_single(&osu).await {
                Ok(result) => match result {
                    Some(user) => user,
                    None => {
                        msg.channel_id
                            .say(&ctx.http, format!("User `{}` was not found", name))
                            .await?;
                        return Ok(());
                    }
                },
                Err(why) => {
                    msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                    return Err(CommandError::from(why.to_string()));
                }
            };
            let scores = match user.get_top_scores(&osu, 100, mode).await {
                Ok(scores) => scores,
                Err(why) => {
                    msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                    return Err(CommandError::from(why.to_string()));
                }
            };
            users.insert(user.user_id, user);
            all_scores.push(scores);
        }
        (users, all_scores)
    };

    // Consider only scores on common maps
    let mut map_ids: HashSet<u32> = all_scores
        .iter()
        .map(|scores| {
            scores
                .iter()
                .map(|s| s.beatmap_id.unwrap())
                .collect::<HashSet<u32>>()
        })
        .flatten()
        .collect();
    map_ids.retain(|&id| {
        all_scores
            .iter()
            .all(|scores| scores.iter().any(|s| s.beatmap_id.unwrap() == id))
    });
    all_scores
        .par_iter_mut()
        .for_each(|scores| scores.retain(|s| map_ids.contains(&s.beatmap_id.unwrap())));

    // Try retrieving all maps of common scores from the database
    let mut maps: HashMap<u32, Beatmap> = {
        let map_ids: Vec<u32> = map_ids.iter().copied().collect();
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql
            .get_beatmaps(&map_ids)
            .unwrap_or_else(|_| HashMap::default())
    };
    let amount_common = map_ids.len();
    debug!("Found {}/{} beatmaps in DB", maps.len(), amount_common);
    map_ids.retain(|id| !maps.contains_key(id));

    // Retrieve all missing maps from the API
    let missing_maps = if !map_ids.is_empty() {
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let mut missing_maps = Vec::with_capacity(map_ids.len());
        for id in map_ids {
            let req = BeatmapRequest::new().map_id(id);
            let map = match req.queue_single(&osu).await {
                Ok(result) => match result {
                    Some(map) => {
                        maps.insert(map.beatmap_id, map.clone());
                        map
                    }
                    None => {
                        msg.channel_id
                            .say(&ctx.http, "Unexpected response from the API, blame bade")
                            .await?;
                        return Ok(());
                    }
                },
                Err(why) => {
                    msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                    return Err(CommandError::from(why.to_string()));
                }
            };
            missing_maps.push(map);
        }
        Some(missing_maps)
    } else {
        None
    };

    // Accumulate all necessary data
    let len = names.len();
    let names_join = names
        .into_iter()
        .collect::<Vec<_>>()
        .chunks(len - 1)
        .map(|chunk| chunk.join("`, `"))
        .join("` and `");
    let mut content = format!("`{}`", names_join);
    if amount_common == 0 {
        content.push_str(" have no common scores");
    } else {
        let _ = write!(
            content,
            " have {} common beatmap{} in their top 100",
            amount_common,
            if amount_common > 1 { "s" } else { "" }
        );
        if amount_common > 10 {
            content.push_str(", here's the top 10 of them:");
        } else {
            content.push(':');
        }
    }
    let (data, thumbnail) = BasicEmbedData::create_common(users, all_scores, maps).await;

    // Creating the embed
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            if !thumbnail.is_empty() {
                let bytes: &[u8] = &thumbnail;
                m.add_file((bytes, "avatar_fuse.png"));
            }
            m.content(content)
                .embed(|e| data.build(e).thumbnail("attachment://avatar_fuse.png"))
        })
        .await;

    // Add missing maps to database
    if let Some(maps) = missing_maps {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.insert_beatmaps(maps) {
            warn!(
                "Could not add missing maps of common command to DB: {}",
                why
            );
        }
    }

    discord::reaction_deletion(&ctx, response?, msg.author.id).await;
    Ok(())
}

#[command]
#[description = "Compare the users' top 100 and check which \
                 maps appear in each top list (up to 10 users)"]
#[usage = "[name1] [name2] ..."]
#[example = "badewanne3 \"nathan on osu\" idke"]
pub async fn common(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    common_send(GameMode::STD, ctx, msg, args).await
}

#[command]
#[description = "Compare the mania users' top 100 and check which \
                 maps appear in each top list (up to 10 users)"]
#[usage = "[name1] [name2] ..."]
#[example = "badewanne3 \"nathan on osu\" idke"]
#[aliases("commonm")]
pub async fn commonmania(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    common_send(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[description = "Compare the taiko users' top 100 and check which \
                 maps appear in each top list (up to 10 users)"]
#[usage = "[name1] [name2] ..."]
#[example = "badewanne3 \"nathan on osu\" idke"]
#[aliases("commont")]
pub async fn commontaiko(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    common_send(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[description = "Compare the ctb users' top 100 and check which \
                 maps appear in each top list (up to 10 users)"]
#[usage = "[name1] [name2] ..."]
#[example = "badewanne3 \"nathan on osu\" idke"]
#[aliases("commonc")]
pub async fn commonctb(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    common_send(GameMode::CTB, ctx, msg, args).await
}
