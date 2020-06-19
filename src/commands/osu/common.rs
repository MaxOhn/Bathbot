use crate::{
    arguments::MultNameArgs,
    database::MySQL,
    embeds::{CommonEmbed, EmbedData},
    pagination::{CommonPagination, Pagination},
    util::{discord, globals::OSU_API_ISSUE, MessageExt},
    DiscordLinks, Osu,
};

use itertools::Itertools;
use rayon::prelude::*;
use rosu::{
    backend::requests::{BeatmapRequest, UserRequest},
    models::{Beatmap, GameMode, Score, User},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
    sync::Arc,
};

#[allow(clippy::cognitive_complexity)]
async fn common_send(mode: GameMode, ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mut args = MultNameArgs::new(args, 10);
    let names = match args.names.len() {
        0 => {
            msg.channel_id
                .say(
                    ctx,
                    "You need to specify at least one osu username. \
                    If you're not linked, you must specify at least two names.",
                )
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Ok(());
        }
        1 => {
            let data = ctx.data.read().await;
            let links = data.get::<DiscordLinks>().unwrap();
            match links.get(msg.author.id.as_u64()) {
                Some(name) => {
                    args.names.insert(name.clone());
                }
                None => {
                    msg.channel_id
                        .say(
                            ctx,
                            "Since you're not linked via `<link`, \
                            you must specify at least two names.",
                        )
                        .await?
                        .reaction_delete(ctx, msg.author.id)
                        .await;
                    return Ok(());
                }
            }
            args.names
        }
        _ => args.names,
    };
    if names.iter().collect::<HashSet<_>>().len() == 1 {
        msg.channel_id
            .say(ctx, "Give at least two different names.")
            .await?
            .reaction_delete(ctx, msg.author.id)
            .await;
        return Ok(());
    }

    // Retrieve all users and their top scores
    let requests: HashMap<String, UserRequest> = names
        .iter()
        .map(|name| (name.clone(), UserRequest::with_username(name).mode(mode)))
        .collect();
    let (users, mut all_scores): (HashMap<u32, User>, Vec<Vec<Score>>) = {
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        let mut users = HashMap::with_capacity(requests.len());
        let mut all_scores = Vec::with_capacity(requests.len());
        for (name, request) in requests.into_iter() {
            let user = match request.queue_single(&osu).await {
                Ok(result) => match result {
                    Some(user) => user,
                    None => {
                        msg.channel_id
                            .say(ctx, format!("User `{}` was not found", name))
                            .await?
                            .reaction_delete(ctx, msg.author.id)
                            .await;
                        return Ok(());
                    }
                },
                Err(why) => {
                    msg.channel_id
                        .say(ctx, OSU_API_ISSUE)
                        .await?
                        .reaction_delete(ctx, msg.author.id)
                        .await;
                    return Err(why.to_string().into());
                }
            };
            let scores = match user.get_top_scores(&osu, 100, mode).await {
                Ok(scores) => scores,
                Err(why) => {
                    msg.channel_id
                        .say(ctx, OSU_API_ISSUE)
                        .await?
                        .reaction_delete(ctx, msg.author.id)
                        .await;
                    return Err(why.to_string().into());
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

    // Flatten scores, sort by beatmap id, then group by beatmap id
    let mut all_scores: Vec<Score> = all_scores.into_iter().flatten().collect();
    all_scores.sort_by(|s1, s2| s1.beatmap_id.cmp(&s2.beatmap_id));
    let mut all_scores: HashMap<u32, Vec<Score>> = all_scores
        .into_iter()
        .group_by(|score| score.beatmap_id.unwrap())
        .into_iter()
        .map(|(map_id, scores)| (map_id, scores.collect()))
        .collect();

    // Sort each group by pp value, then take the best 3
    all_scores.par_iter_mut().for_each(|(_, scores)| {
        scores.sort_by(|s1, s2| s2.pp.partial_cmp(&s1.pp).unwrap());
        scores.truncate(3);
    });

    // Consider only the top 10 maps with the highest avg pp among the users
    let mut pp_avg: Vec<(u32, f32)> = all_scores
        .par_iter()
        .map(|(&map_id, scores)| {
            let sum = scores.iter().fold(0.0, |sum, next| sum + next.pp.unwrap());
            (map_id, sum / scores.len() as f32)
        })
        .collect();
    pp_avg.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // Try retrieving all maps of common scores from the database
    let mut maps: HashMap<u32, Beatmap> = {
        let map_ids: Vec<u32> = map_ids.iter().copied().collect();
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        mysql
            .get_beatmaps(&map_ids)
            .await
            .unwrap_or_else(|_| HashMap::default())
    };
    let amount_common = map_ids.len();
    map_ids.retain(|id| !maps.contains_key(id));

    // Retrieve all missing maps from the API
    let missing_maps = if !map_ids.is_empty() {
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
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
                            .say(ctx, "Unexpected response from the API, blame bade")
                            .await?
                            .reaction_delete(ctx, msg.author.id)
                            .await;
                        return Ok(());
                    }
                },
                Err(why) => {
                    msg.channel_id
                        .say(ctx, OSU_API_ISSUE)
                        .await?
                        .reaction_delete(ctx, msg.author.id)
                        .await;
                    return Err(why.to_string().into());
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
    }

    // Keys have no strict order, hence inconsistent result
    let user_ids: Vec<u32> = users.keys().copied().collect();
    let thumbnail = match discord::get_combined_thumbnail(&user_ids).await {
        Ok(thumbnail) => thumbnail,
        Err(why) => {
            warn!("Error while combining avatars: {}", why);
            Vec::default()
        }
    };
    let id_pps = &pp_avg[..10.min(pp_avg.len())];
    let data = CommonEmbed::new(&users, &all_scores, &maps, id_pps, 0);

    // Creating the embed
    let resp = msg
        .channel_id
        .send_message(ctx, |m| {
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
        let mysql = data.get::<MySQL>().unwrap();
        let len = maps.len();
        match mysql.insert_beatmaps(maps).await {
            Ok(_) if len == 1 => {}
            Ok(_) => info!("Added {} maps to DB", len),
            Err(why) => warn!("Error while adding maps to DB: {}", why),
        }
    }

    // Skip pagination if too few entries
    if pp_avg.len() <= 10 {
        resp?.reaction_delete(ctx, msg.author.id).await;
        return Ok(());
    }

    // Pagination
    let pagination = CommonPagination::new(
        ctx,
        resp?,
        msg.author.id,
        users,
        all_scores,
        maps,
        pp_avg,
        "attachment://avatar_fuse.png".to_owned(),
    )
    .await;
    let cache = Arc::clone(&ctx.cache);
    let http = Arc::clone(&ctx.http);
    tokio::spawn(async move {
        if let Err(why) = pagination.start(cache, http).await {
            warn!("Pagination error: {}", why)
        }
    });
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
