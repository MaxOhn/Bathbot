use crate::{
    arguments::NameIntArgs,
    database::MySQL,
    embeds::BasicEmbedData,
    pagination::{NoChokePagination, Pagination},
    util::{globals::OSU_API_ISSUE, numbers, osu, pp::PPProvider, MessageExt},
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::UserRequest,
    models::{Beatmap, GameMode, Score, User},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::{channel::Message, misc::Mentionable},
    prelude::Context,
};
use std::{cmp::Ordering, collections::HashMap, sync::Arc};

#[command]
#[description = "Display a user's top plays if no score in their top 100 \
                 would be a choke.\nIf a number is specified, \
                 I will only unchoke scores with at most that many misses"]
#[usage = "[username] [number for miss limit]"]
#[example = "badewanne3"]
#[example = "vaxei 5"]
#[aliases("nc", "nochoke")]
async fn nochokes(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let args = NameIntArgs::new(args);
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
                        &ctx.http,
                        "Either specify an osu name or link your discord \
                     to an osu profile via `<link osuname`",
                    )
                    .await?;
                return Ok(());
            }
        }
    };
    let miss_limit = args.number;

    // Retrieve the user and its top scores
    let (user, scores): (User, Vec<Score>) = {
        let user_req = UserRequest::with_username(&name).mode(GameMode::STD);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        let user = match user_req.queue_single(&osu).await {
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
        let scores = match user.get_top_scores(&osu, 100, GameMode::STD).await {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        (user, scores)
    };

    // Get all relevant maps from the database
    let map_ids: Vec<u32> = scores.iter().map(|s| s.beatmap_id.unwrap()).collect();
    let mut maps = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        mysql
            .get_beatmaps(&map_ids)
            .unwrap_or_else(|_| HashMap::default())
    };
    debug!("Found {}/{} beatmaps in DB", maps.len(), scores.len());

    let retrieving_msg = if scores.len() - maps.len() > 10 {
        Some(
            msg.channel_id
                .say(
                    &ctx.http,
                    format!(
                        "Retrieving {} maps from the api...",
                        scores.len() - maps.len()
                    ),
                )
                .await?,
        )
    } else {
        None
    };

    // Further prepare data and retrieve missing maps
    let (scores_data, missing_maps): (Vec<_>, Option<Vec<Beatmap>>) = {
        let mut scores_data = Vec::with_capacity(scores.len());
        let mut missing_maps = Vec::new();
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        for (i, score) in scores.into_iter().enumerate() {
            let map_id = score.beatmap_id.unwrap();
            let map = if maps.contains_key(&map_id) {
                maps.remove(&map_id).unwrap()
            } else {
                let map = match score.get_beatmap(osu).await {
                    Ok(map) => map,
                    Err(why) => {
                        msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                        return Err(CommandError::from(why.to_string()));
                    }
                };
                missing_maps.push(map.clone());
                map
            };

            // Unchoke the score
            let mut unchoked = score.clone();
            if score.max_combo != map.max_combo.unwrap()
                && (miss_limit.is_none() || score.count_miss <= *miss_limit.as_ref().unwrap())
            {
                osu::unchoke_score(&mut unchoked, &map);
                let pp = PPProvider::calculate_oppai_pp(&unchoked, &map).await?;
                unchoked.pp = Some(pp);
            }
            scores_data.push((i + 1, score, unchoked, map));
        }
        let missing_maps = if missing_maps.is_empty() {
            None
        } else {
            Some(missing_maps)
        };

        // Sort by unchoked pp
        scores_data.sort_by(|(_, _, s1, _), (_, _, s2, _)| {
            s2.pp
                .unwrap()
                .partial_cmp(&s1.pp.unwrap())
                .unwrap_or_else(|| Ordering::Equal)
        });
        (scores_data, missing_maps)
    };

    // Calculate total user pp without chokes
    let mut factor: f64 = 1.0;
    let mut actual_pp = 0.0;
    let mut unchoked_pp = 0.0;
    for (idx, actual, unchoked, _) in scores_data.iter() {
        actual_pp += actual.pp.unwrap() as f64 * 0.95_f64.powi(*idx as i32 - 1);
        unchoked_pp += factor * unchoked.pp.unwrap() as f64;
        factor *= 0.95;
    }
    let bonus_pp = user.pp_raw as f64 - actual_pp;
    unchoked_pp += bonus_pp;
    unchoked_pp = (100.0 * unchoked_pp).round() / 100.0;

    // Accumulate all necessary data
    let pages = numbers::div_euclid(5, scores_data.len());
    let data = match BasicEmbedData::create_nochoke(
        &user,
        scores_data.iter().take(5),
        unchoked_pp,
        (1, pages),
        &ctx.cache,
    )
    .await
    {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id
                .say(
                    &ctx.http,
                    "Some issue while calculating nochoke data, blame bade",
                )
                .await?;
            return Err(CommandError::from(why.to_string()));
        }
    };
    let mention = msg.author.mention();

    if let Some(msg) = retrieving_msg {
        msg.delete(&ctx.http).await?;
    }

    // Creating the embed
    let resp = msg
        .channel_id
        .send_message(ctx, |m| {
            m.content(format!("{} No-choke top scores for `{}`:", mention, name))
                .embed(|e| data.build(e))
        })
        .await;

    // Add missing maps to database
    if let Some(maps) = missing_maps {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        if let Err(why) = mysql.insert_beatmaps(maps) {
            warn!(
                "Could not add missing maps of nochoke command to DB: {}",
                why
            );
        }
    }

    // Skip pagination if too few entries
    if scores_data.len() <= 5 {
        resp?.reaction_delete(ctx, msg.author.id).await;
        return Ok(());
    }

    // Pagination
    let pagination =
        NoChokePagination::new(ctx, resp?, msg.author.id, user, scores_data, unchoked_pp).await;
    let cache = Arc::clone(&ctx.cache);
    let http = Arc::clone(&ctx.http);
    tokio::spawn(async move {
        if let Err(why) = pagination.start(cache, http).await {
            warn!("Pagination error: {}", why)
        }
    });
    Ok(())
}
