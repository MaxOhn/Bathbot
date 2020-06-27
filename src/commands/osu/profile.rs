use crate::{
    arguments::NameArgs,
    scraper::{OsuStatsScore, OsuStatsParams},
    database::MySQL,
    embeds::{EmbedData, ProfileEmbed},
    util::{globals::OSU_API_ISSUE, MessageExt},
    DiscordLinks, Osu, Scraper,
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

#[allow(clippy::cognitive_complexity)]
async fn profile_send(mode: GameMode, ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let args = NameArgs::new(args);
    let name = if let Some(name) = args.name {
        name
    } else {
        let data = ctx.data.read().await;
        let links = data.get::<DiscordLinks>().unwrap();
        match links.get(msg.author.id.as_u64()) {
            Some(name) => name.clone(),
            None => {
                msg.channel_id
                    .say(
                        ctx,
                        "Either specify an osu name or link your discord \
                        to an osu profile via `<link osuname`",
                    )
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Ok(());
            }
        }
    };

    // Retrieve the user and its top scores
    let (user, scores): (User, Vec<Score>) = {
        let user_req = UserRequest::with_username(&name).mode(mode);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        let user = match user_req.queue_single(&osu).await {
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
        (user, scores)
    };

    let (map_process_result, globals_count) = tokio::try_join!(
        process_maps(ctx, &scores),
        get_globals_count(ctx, user.username.clone(), mode)
    ).await;
    let (score_maps, missing_maps, retrieving_msg) = match map_process_result {
        Ok(results) => results,
        Err(why) => {
            msg.channel_id
                .say(ctx, OSU_API_ISSUE)
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Err(why);
        },
    }

    // Accumulate all necessary data
    let data = ProfileEmbed::new(user, score_maps, mode, global_counts, &ctx.cache).await;

    if let Some(msg) = retrieving_msg {
        msg.delete(ctx).await?;
    }

    // Send the embed
    let response = msg
        .channel_id
        .send_message(ctx, |m| m.embed(|e| data.build(e)))
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
    response?.reaction_delete(ctx, msg.author.id).await;
    Ok(())
}

async fn process_maps(ctx: &Context, scores: &[Score]) -> Result<(Vec<(Score, Beatmap)>, Option<Vec<Beatmap>>, Option<Message>), CommandError> {
    // Get all relevant maps from the database
    let map_ids: Vec<u32> = scores.iter().map(|s| s.beatmap_id.unwrap()).collect();
    let mut maps = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        mysql
            .get_beatmaps(&map_ids)
            .await
            .unwrap_or_else(|_| HashMap::default())
    };
    debug!("Found {}/{} beatmaps in DB", maps.len(), scores.len());

    let retrieving_msg = if scores.len() - maps.len() > 15 {
        msg.channel_id
            .say(
                ctx,
                format!(
                    "Retrieving {} maps from the api...",
                    scores.len() - maps.len()
                ),
            )
            .await.ok()
    } else {
        None
    };

    // Retrieving all missing beatmaps
    let (score_maps, missing_indices) = {
        let mut tuples = Vec::with_capacity(scores.len());
        let mut missing_indices = Vec::with_capacity(scores.len());
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        for (i, score) in scores.into_iter().enumerate() {
            let map_id = score.beatmap_id.unwrap();
            let map = if maps.contains_key(&map_id) {
                maps.remove(&map_id).unwrap()
            } else {
                missing_indices.push(i);
                score.get_beatmap(osu).await.or_else(|why| {
                    format_err!("Error while retrieving Beatmap of score: {}", why)
                })?
            };
            tuples.push((score, map));
        }
        (tuples, missing_indices)
    };
    let missing_maps: Option<Vec<Beatmap>> = if missing_indices.is_empty() {
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
    Ok((score_maps, missing_maps, retrieving_msg))
}

async fn get_globals_count(ctx: &Context, name: String, mode: GameMode) -> HashMap<usize, usize> {
    let data = ctx.data.read().await;
    let scraper = data.get::<Scraper>().unwrap();
    let mut counts = HashMap::new();
    let mut params = OsuStatsParams::new(name).mode(mode);
    for rank in [50, 20, 10, 5, 1] {
        params = params.rank_max(rank);
        match scraper.get_global_scores(&params).await {
            Ok((_, count)) => counts.insert(rank, count),
            Err(why) => error!("Error while retrieving osustats for profile: {}", why),
        }
    }
    counts
}

#[command]
#[description = "Display statistics of a user"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("osu")]
pub async fn profile(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    profile_send(GameMode::STD, ctx, msg, args).await
}

#[command]
#[description = "Display statistics of a mania user"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("mania", "maniaprofile", "profilem")]
pub async fn profilemania(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    profile_send(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[description = "Display statistics of a taiko user"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("taiko", "taikoprofile", "profilet")]
pub async fn profiletaiko(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    profile_send(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[description = "Display statistics of ctb user"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("ctb", "ctbprofile", "profilec")]
pub async fn profilectb(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    profile_send(GameMode::CTB, ctx, msg, args).await
}
