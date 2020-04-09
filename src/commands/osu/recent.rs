use crate::{
    arguments::NameArgs, database::MySQL, embeds::RecentData, util::globals::OSU_API_ISSUE,
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::{RecentRequest, UserRequest},
    models::{
        ApprovalStatus::{Approved, Loved, Qualified, Ranked},
        Beatmap, GameMode, Score, User,
    },
    OsuError,
};
use serenity::{
    collector::{ReactionAction, ReactionCollectorBuilder},
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::channel::{Message, ReactionType},
    prelude::Context,
};
use std::collections::{HashMap, HashSet};
use tokio::time::Duration;

#[allow(clippy::cognitive_complexity)]
async fn recent_send(
    mode: GameMode,
    ctx: &mut Context,
    msg: &Message,
    args: Args,
) -> CommandResult {
    let args = NameArgs::new(args);
    let name = if let Some(name) = args.name {
        name
    } else {
        let data = ctx.data.read().await;
        let links = data
            .get::<DiscordLinks>()
            .expect("Could not get DiscordLinks");
        match links.get(msg.author.id.as_u64()) {
            Some(name) => name.clone(),
            None => {
                msg.channel_id
                    .say(
                        &ctx.http,
                        "Either specify an osu name or link your discord \
                     to an osu profile via `<link osuname`",
                    )
                    .await?;
                return Ok(());
            }
        }
    };

    // Retrieve the recent scores
    let scores = {
        let request = RecentRequest::with_username(&name).mode(mode).limit(50);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        match request.queue(osu).await {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };
    if scores.is_empty() {
        msg.channel_id
            .say(
                &ctx.http,
                format!("No recent plays found for user `{}`", name),
            )
            .await?;
        return Ok(());
    }

    // Retrieving the score's user
    let user = {
        let req = UserRequest::with_username(&name).mode(mode);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        match req.queue_single(&osu).await {
            Ok(Some(u)) => u,
            Ok(None) => unreachable!(),
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };

    // Get all relevant maps from the database
    let map_ids: HashSet<u32> = scores.iter().map(|s| s.beatmap_id.unwrap()).collect();
    let mut maps = {
        let dedubed_ids: Vec<u32> = map_ids.iter().copied().collect();
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql
            .get_beatmaps(&dedubed_ids)
            .unwrap_or_else(|_| HashMap::default())
    };
    info!(
        "Found {}/{} beatmaps in the database",
        maps.len(),
        map_ids.len()
    );
    let retrieving_msg = if scores.len() - maps.len() > 15 {
        Some(
            msg.channel_id
                .say(
                    &ctx.http,
                    format!(
                        "Retrieving {} maps from the api...",
                        map_ids.len() - maps.len()
                    ),
                )
                .await?,
        )
    } else {
        None
    };

    // Retrieving all missing beatmaps
    let mut missing_ids = Vec::with_capacity(map_ids.len());
    #[allow(clippy::map_entry)]
    {
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        for score in scores.iter() {
            let map_id = score.beatmap_id.unwrap();
            if !maps.contains_key(&map_id) {
                let map = match score.get_beatmap(osu).await {
                    Ok(map) => map,
                    Err(why) => {
                        msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                        return Err(CommandError::from(why.to_string()));
                    }
                };
                match map.approval_status {
                    Ranked | Loved => missing_ids.push(map_id),
                    _ => {}
                }
                maps.insert(map_id, map);
            };
        }
    }

    // Retrieving the user's top 100 and the map's global top 50
    let best = {
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        match user.get_top_scores(osu, 100, mode).await {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };
    let mut global = HashMap::with_capacity(50);
    let first_score = scores.first().unwrap();
    let first_id = first_score.beatmap_id.unwrap();
    let first_map = maps.get(&first_id).unwrap();
    {
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        match first_map.approval_status {
            Ranked | Loved | Qualified | Approved => {
                match first_map.get_global_leaderboard(osu, 50).await {
                    Ok(scores) => {
                        global.insert(first_map.beatmap_id, scores);
                    }
                    Err(why) => {
                        msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                        return Err(CommandError::from(why.to_string()));
                    }
                }
            }
            _ => {}
        }
    }

    // Accumulate all necessary data
    let tries = scores
        .iter()
        .take_while(|s| s.beatmap_id.unwrap() == first_id)
        .count();
    let mut embed_data = match RecentData::new(
        &user,
        first_score,
        first_map,
        &best,
        global.get(&first_map.beatmap_id).unwrap(),
        &ctx,
    )
    .await
    {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id
                .say(
                    &ctx.http,
                    "Some issue while calculating recent data, blame bade",
                )
                .await?;
            return Err(CommandError::from(why.to_string()));
        }
    };

    if let Some(msg) = retrieving_msg {
        msg.delete(&ctx.http).await?;
    }

    // Creating the embed
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.content(format!("Try #{}", tries))
                .embed(|e| embed_data.build(e))
        })
        .await;

    // Add missing maps to database
    if !missing_ids.is_empty() {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        let missing_maps: Vec<Beatmap> = missing_ids
            .into_iter()
            .map(|id| maps.get(&id).unwrap().clone())
            .collect();
        if let Err(why) = mysql.insert_beatmaps(missing_maps) {
            warn!(
                "Could not add missing maps of top command to database: {}",
                why
            );
        }
    }
    let mut response = response?;

    // Collect reactions of author on the response
    let mut collector = ReactionCollectorBuilder::new(&ctx)
        .author_id(msg.author.id)
        .message_id(response.id)
        .timeout(Duration::from_secs(60))
        .await;
    let mut idx = 0;

    // Add initial reactions
    let reactions = ["⏮️", "⏪", "◀️", "▶️", "⏩", "⏭️"];
    for &reaction in reactions.iter() {
        response.react(&ctx.http, reaction).await?;
    }

    // Check if the author wants to edit the response
    while let Some(reaction) = collector.receive_one().await {
        if let ReactionAction::Added(reaction) = &*reaction {
            if let ReactionType::Unicode(reaction_name) = &reaction.emoji {
                let reaction_data = reaction_data(
                    reaction_name.as_str(),
                    &mut idx,
                    &user,
                    &scores,
                    &maps,
                    &best,
                    &mut global,
                    &ctx,
                );
                match reaction_data.await {
                    Ok(ReactionData::None) => {}
                    Ok(ReactionData::Delete) => response.delete(&ctx).await?,
                    Ok(ReactionData::Data { data, idx }) => {
                        let content = format!("Recent score #{}", idx + 1);
                        embed_data = *data;
                        response
                            .edit(&ctx, |m| m.content(content).embed(|e| embed_data.build(e)))
                            .await?
                    }
                    Err(why) => {
                        msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                        return Err(CommandError::from(why.to_string()));
                    }
                }
            }
        }
    }
    for &reaction in reactions.iter() {
        response
            .channel_id
            .delete_reaction(&ctx.http, response.id, None, reaction)
            .await?;
    }

    // Minimize embed
    response
        .edit(&ctx, |m| m.embed(|e| embed_data.minimize(e)))
        .await?;
    Ok(())
}

enum ReactionData {
    Data { data: Box<RecentData>, idx: usize },
    Delete,
    None,
}

#[allow(clippy::too_many_arguments)]
async fn reaction_data(
    reaction: &str,
    idx: &mut usize,
    user: &User,
    scores: &[Score],
    maps: &HashMap<u32, Beatmap>,
    best: &[Score],
    global: &mut HashMap<u32, Vec<Score>>,
    ctx: &Context,
) -> Result<ReactionData, OsuError> {
    let amount = scores.len();
    let data = match reaction {
        "❌" => ReactionData::Delete,
        "⏮️" => {
            if *idx > 0 {
                *idx = 0;
                let score = scores.first().unwrap();
                let map = maps.get(&score.beatmap_id.unwrap()).unwrap();
                let global_lb = global_lb(ctx, map, global).await?;
                RecentData::new(&user, score, map, best, global_lb, ctx)
                    .await
                    .map(|data| ReactionData::Data {
                        data: Box::new(data),
                        idx: *idx,
                    })
                    .unwrap_or_else(|why| {
                        warn!(
                            "Error editing recent data at idx {}/{}: {}",
                            idx, amount, why
                        );
                        ReactionData::None
                    })
            } else {
                ReactionData::None
            }
        }
        "⏪" => {
            if *idx > 0 {
                *idx = idx.saturating_sub(5);
                let score = scores.get(*idx).unwrap();
                let map = maps.get(&score.beatmap_id.unwrap()).unwrap();
                let global_lb = global_lb(ctx, map, global).await?;
                RecentData::new(&user, score, map, best, global_lb, ctx)
                    .await
                    .map(|data| ReactionData::Data {
                        data: Box::new(data),
                        idx: *idx,
                    })
                    .unwrap_or_else(|why| {
                        warn!(
                            "Error editing recent data at idx {}/{}: {}",
                            idx, amount, why
                        );
                        ReactionData::None
                    })
            } else {
                ReactionData::None
            }
        }
        "◀️" => {
            if *idx > 0 {
                *idx = idx.saturating_sub(1);
                let score = scores.get(*idx).unwrap();
                let map = maps.get(&score.beatmap_id.unwrap()).unwrap();
                let global_lb = global_lb(ctx, map, global).await?;
                RecentData::new(&user, score, map, best, global_lb, ctx)
                    .await
                    .map(|data| ReactionData::Data {
                        data: Box::new(data),
                        idx: *idx,
                    })
                    .unwrap_or_else(|why| {
                        warn!(
                            "Error editing recent data at idx {}/{}: {}",
                            idx, amount, why
                        );
                        ReactionData::None
                    })
            } else {
                ReactionData::None
            }
        }
        "▶️" => {
            let limit = amount.saturating_sub(1);
            if *idx < limit {
                *idx = limit.min(*idx + 1);
                let score = scores.get(*idx).unwrap();
                let map = maps.get(&score.beatmap_id.unwrap()).unwrap();
                let global_lb = global_lb(ctx, map, global).await?;
                RecentData::new(&user, score, map, best, global_lb, ctx)
                    .await
                    .map(|data| ReactionData::Data {
                        data: Box::new(data),
                        idx: *idx,
                    })
                    .unwrap_or_else(|why| {
                        warn!(
                            "Error editing recent data at idx {}/{}: {}",
                            idx, amount, why
                        );
                        ReactionData::None
                    })
            } else {
                ReactionData::None
            }
        }
        "⏩" => {
            let limit = amount.saturating_sub(5);
            if *idx < limit {
                *idx = limit.min(*idx + 5);
                let score = scores.get(*idx).unwrap();
                let map = maps.get(&score.beatmap_id.unwrap()).unwrap();
                let global_lb = global_lb(ctx, map, global).await?;
                RecentData::new(&user, score, map, best, global_lb, ctx)
                    .await
                    .map(|data| ReactionData::Data {
                        data: Box::new(data),
                        idx: *idx,
                    })
                    .unwrap_or_else(|why| {
                        warn!(
                            "Error editing recent data at idx {}/{}: {}",
                            idx, amount, why
                        );
                        ReactionData::None
                    })
            } else {
                ReactionData::None
            }
        }
        "⏭️" => {
            let limit = amount.saturating_sub(5);
            if *idx < limit {
                *idx = limit;
                let score = scores.get(*idx).unwrap();
                let map = maps.get(&score.beatmap_id.unwrap()).unwrap();
                let global_lb = global_lb(ctx, map, global).await?;
                RecentData::new(&user, score, map, best, global_lb, ctx)
                    .await
                    .map(|data| ReactionData::Data {
                        data: Box::new(data),
                        idx: *idx,
                    })
                    .unwrap_or_else(|why| {
                        warn!(
                            "Error editing recent data at idx {}/{}: {}",
                            idx, amount, why
                        );
                        ReactionData::None
                    })
            } else {
                ReactionData::None
            }
        }
        _ => ReactionData::None,
    };
    Ok(data)
}

#[allow(clippy::map_entry)]
async fn global_lb<'g>(
    ctx: &Context,
    map: &Beatmap,
    global: &'g mut HashMap<u32, Vec<Score>>,
) -> Result<&'g [Score], OsuError> {
    if !global.contains_key(&map.beatmap_id) {
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get Osu");
        let global_lb = map.get_global_leaderboard(&osu, 50).await?;
        global.insert(map.beatmap_id, global_lb);
    };
    Ok(global.get(&map.beatmap_id).unwrap())
}

#[command]
#[description = "Display a user's most recent play"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("r", "rs")]
pub async fn recent(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::STD, ctx, msg, args).await
}

#[command]
#[description = "Display a user's most recent mania play"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("rm")]
pub async fn recentmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[description = "Display a user's most recent taiko play"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("rt")]
pub async fn recenttaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[description = "Display a user's most recent ctb play"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("rc")]
pub async fn recentctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::CTB, ctx, msg, args).await
}
