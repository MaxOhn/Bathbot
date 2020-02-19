use crate::{
    database::MySQL,
    messages::{BotEmbed, CommonData},
    util::globals::OSU_API_ISSUE,
    DiscordLinks, Error, Osu,
};

use itertools::Itertools;
use rosu::{
    backend::requests::{BeatmapArgs, OsuArgs, OsuRequest, UserArgs, UserBestArgs},
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
};
use tokio::runtime::Runtime;

fn common_send(mode: GameMode, ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse the names
    if args.is_empty() {
        msg.channel_id.say(
            &ctx.http,
            "You need to specify at least one osu username. \
             If you're not linked, you must specify at least two names.",
        )?;
        return Ok(());
    }
    let mut names = Vec::with_capacity(args.len());
    while !args.is_empty() {
        names.push(args.trimmed().single_quoted::<String>()?);
    }
    if names.len() == 1 {
        let data = ctx.data.read();
        let links = data
            .get::<DiscordLinks>()
            .expect("Could not get DiscordLinks");
        match links.get(msg.author.id.as_u64()) {
            Some(name) => {
                names.push(name.clone());
            }
            None => {
                msg.channel_id.say(
                    &ctx.http,
                    "You need to specify at least one osu username. \
                     If you're not linked, you must specify at least two names.",
                )?;
                return Ok(());
            }
        }
    }

    // Prepare all requests
    let mut args = HashMap::with_capacity(names.len());
    for name in names.iter() {
        args.insert(
            name.clone(),
            (
                UserArgs::with_username(name).mode(mode),
                UserBestArgs::with_username(name).mode(mode).limit(100),
            ),
        );
    }
    let reqs: HashMap<String, (OsuRequest<User>, OsuRequest<Score>)> = {
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        args.into_iter()
            .map(|(name, (user_arg, best_arg))| {
                (
                    name,
                    (
                        osu.create_request(OsuArgs::Users(user_arg)),
                        osu.create_request(OsuArgs::Best(best_arg)),
                    ),
                )
            })
            .collect()
    };
    let mut rt = Runtime::new().unwrap();

    // Retrieve users and their topscores
    let res = rt.block_on(async {
        let mut user_scores = HashMap::with_capacity(reqs.len());
        for (name, (users_req, scores_req)) in reqs.into_iter() {
            let users = users_req
                .queue()
                .await
                .or_else(|e| Err(CommandError(format!("Error while retrieving Users: {}", e))))?;
            let scores = scores_req.queue().await.or_else(|e| {
                Err(CommandError(format!(
                    "Error while retrieving UserBest: {}",
                    e
                )))
            })?;
            user_scores.insert(name, (users, scores));
        }
        Ok(user_scores)
    });
    let (users, mut all_scores): (HashMap<u32, User>, Vec<Vec<Score>>) = match res {
        Ok(mut user_scores) => {
            for (name, (users, _)) in user_scores.iter() {
                if users.is_empty() {
                    msg.channel_id
                        .say(&ctx.http, format!("User `{}` was not found", name))?;
                    return Ok(());
                }
            }
            user_scores
                .drain()
                .map(|(_, (mut users, scores))| {
                    let user = users.pop().unwrap();
                    ((user.user_id, user), scores)
                })
                .unzip()
        }
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
        }
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
        .iter_mut()
        .for_each(|scores| scores.retain(|s| map_ids.contains(&s.beatmap_id.unwrap())));

    // Try retrieving all maps of common scores from the database
    let mut maps: HashMap<u32, Beatmap> = {
        let map_ids: Vec<u32> = map_ids.iter().copied().collect();
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql
            .get_beatmaps(&map_ids)
            .unwrap_or_else(|_| HashMap::default())
    };
    let amount_common = map_ids.len();
    info!(
        "Found {}/{} beatmaps in the database",
        maps.len(),
        amount_common
    );
    map_ids.retain(|id| !maps.contains_key(id));

    // Retrieve all missing maps from the API
    let missing_maps = if !map_ids.is_empty() {
        let map_reqs: Vec<OsuRequest<Beatmap>> = {
            let data = ctx.data.read();
            let osu = data.get::<Osu>().expect("Could not get osu client");
            map_ids
                .iter()
                .filter(|id| !maps.contains_key(id))
                .map(|&id| osu.create_request(OsuArgs::Beatmaps(BeatmapArgs::new().map_id(id))))
                .collect()
        };
        let res = rt.block_on(async {
            let capacity = map_ids.len();
            let mut missing_maps = Vec::with_capacity(capacity);
            for req in map_reqs.into_iter() {
                match req.queue().await?.pop() {
                    Some(map) => missing_maps.push(map),
                    None => {
                        return Err(Error::Custom("Got zero elements from API".to_string()));
                    }
                }
            }
            Ok(missing_maps)
        });
        match res {
            Ok(missing_maps) => {
                for map in missing_maps.iter() {
                    maps.insert(map.beatmap_id, map.clone());
                }
                Some(missing_maps)
            }
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    } else {
        None
    };

    // Accumulate all necessary data
    let names_join = names
        .chunks(names.len() - 1)
        .map(|chunk| chunk.join("`, `"))
        .join("` and `");
    let mut content = format!("`{}`", names_join);
    if amount_common == 0 {
        content.push_str(" have no common scores");
    } else {
        content.push_str(&format!(
            " have {} common beatmaps in their top 100",
            amount_common
        ));
        if amount_common > 10 {
            content.push_str(", here's the top 10 of them:");
        } else {
            content.push(':');
        }
    }
    let (data, thumbnail) = CommonData::new(users, all_scores, maps);

    // Creating the embed
    let embed = BotEmbed::UserCommonScores(data);
    let _ = msg.channel_id.send_message(&ctx.http, |m| {
        if !thumbnail.is_empty() {
            let bytes: &[u8] = &thumbnail;
            m.add_file((bytes, "avatar_fuse.png"));
        }
        m.content(content).embed(|e| embed.create(e))
    });

    // Add missing maps to database
    if let Some(maps) = missing_maps {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.insert_beatmaps(maps) {
            warn!(
                "Could not add missing maps of common command to database: {}",
                why
            );
        }
    }
    Ok(())
}

#[command]
#[description = "Compare the users' top 100 and check which maps appear in each top list"]
#[usage = "badewanne3 \"nathan on osu\" idke"]
pub fn common(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    common_send(GameMode::STD, ctx, msg, args)
}

#[command]
#[description = "Compare the mania users' top 100 and check which maps appear in each top list"]
#[usage = "badewanne3 \"nathan on osu\" idke"]
#[aliases("commonm")]
pub fn commonmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    common_send(GameMode::MNA, ctx, msg, args)
}

#[command]
#[description = "Compare the taiko users' top 100 and check which maps appear in each top list"]
#[usage = "badewanne3 \"nathan on osu\" idke"]
#[aliases("commont")]
pub fn commontaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    common_send(GameMode::TKO, ctx, msg, args)
}

#[command]
#[description = "Compare the ctb users' top 100 and check which maps appear in each top list"]
#[usage = "badewanne3 \"nathan on osu\" idke"]
#[aliases("commonc")]
pub fn commonctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    common_send(GameMode::CTB, ctx, msg, args)
}
