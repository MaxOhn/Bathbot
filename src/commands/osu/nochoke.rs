use crate::{
    arguments::NameIntArgs,
    embeds::{EmbedData, NoChokeEmbed},
    pagination::{NoChokePagination, Pagination},
    pp::{Calculations, PPCalculator},
    tracking::process_tracking,
    unwind_error,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        numbers, osu, MessageExt,
    },
    Args, BotResult, Context, Error,
};

use futures::future::try_join_all;
use rosu::model::GameMode;
use std::{cmp::Ordering, collections::HashMap, sync::Arc};
use twilight_model::channel::Message;

#[command]
#[short_desc("Unchoke a user's top100")]
#[long_desc(
    "Display a user's top plays if no score in their top100 \
     would be a choke.\nIf a number is specified, \
     I will only unchoke scores with at most that many misses"
)]
#[usage("[username] [number for miss limit]")]
#[example("badewanne3", "vaxei 5")]
#[aliases("nc", "nochoke")]
async fn nochokes(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = NameIntArgs::new(&ctx, args);
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };
    let miss_limit = args.number;

    // Retrieve the user and their top scores
    let user_fut = ctx.osu().user(name.as_str()).mode(GameMode::STD);
    let scores_fut = ctx
        .osu()
        .top_scores(name.as_str())
        .mode(GameMode::STD)
        .limit(100);
    let join_result = tokio::try_join!(user_fut, scores_fut);
    let (user, scores) = match join_result {
        Ok((Some(user), scores)) => (user, scores),
        Ok((None, _)) => {
            let content = format!("User `{}` was not found", name);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };

    // Process user and their top scores for tracking
    let mut maps = HashMap::new();
    process_tracking(&ctx, GameMode::STD, &scores, Some(&user), &mut maps).await;

    // Get all relevant maps from the database
    let map_ids: Vec<u32> = scores.iter().filter_map(|s| s.beatmap_id).collect();
    let mut maps = match ctx.psql().get_beatmaps(&map_ids).await {
        Ok(maps) => maps,
        Err(why) => {
            unwind_error!(warn, why, "Error while getting maps from DB: {}");
            HashMap::default()
        }
    };
    debug!("Found {}/{} beatmaps in DB", maps.len(), scores.len());

    let retrieving_msg = if scores.len() - maps.len() > 10 {
        let content = format!(
            "Retrieving {} maps from the api...",
            scores.len() - maps.len()
        );
        let response = ctx
            .http
            .create_message(msg.channel_id)
            .content(content)?
            .await?;
        Some(response)
    } else {
        None
    };

    // Further prepare data and retrieve missing maps
    let mut scores_data = Vec::with_capacity(scores.len());
    let mut missing_maps = Vec::new();
    for (i, score) in scores.into_iter().enumerate() {
        let map_id = score.beatmap_id.unwrap();
        let map = if let Some(map) = maps.remove(&map_id) {
            map
        } else {
            let map = match ctx.osu().beatmap().map_id(map_id).await {
                Ok(Some(map)) => map,
                Ok(None) => {
                    let content = format!("The API returned no beatmap for map id {}", map_id);
                    return msg.error(&ctx, content).await;
                }
                Err(why) => {
                    let _ = msg.error(&ctx, OSU_API_ISSUE).await;
                    return Err(why.into());
                }
            };
            missing_maps.push(map.clone());
            map
        };
        scores_data.push((i + 1, score, map));
    }

    // Unchoke scores
    let unchoke_fut = scores_data.into_iter().map(|(i, score, map)| async move {
        let mut unchoked = score.clone();
        if score.max_combo != map.max_combo.unwrap_or(0)
            && miss_limit.map_or(true, |l| score.count_miss <= l)
        {
            osu::unchoke_score(&mut unchoked, &map);
            let pp = {
                let mut calculator = PPCalculator::new().score(&unchoked).map(&map);
                calculator.calculate(Calculations::PP).await?;
                calculator.pp()
            };
            unchoked.pp =
                pp.and_then(|pp_unchoke| score.pp.map(|pp_actual| pp_actual.max(pp_unchoke)));
        }
        Ok::<_, Error>((i, score, unchoked, map))
    });
    let mut scores_data = match try_join_all(unchoke_fut).await {
        Ok(scores_data) => scores_data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            return Err(why);
        }
    };

    // Calculate bonus pp
    let actual_pp = scores_data
        .iter()
        .map(|(i, s, ..)| s.pp.unwrap_or(0.0) as f64 * 0.95_f64.powi(*i as i32 - 1))
        .sum::<f64>();
    let bonus_pp = user.pp_raw as f64 - actual_pp;

    // Sort by unchoked pp
    scores_data.sort_unstable_by(|(_, _, s1, _), (_, _, s2, _)| {
        s2.pp.partial_cmp(&s1.pp).unwrap_or(Ordering::Equal)
    });

    // Calculate total user pp without chokes
    let mut unchoked_pp = scores_data
        .iter()
        .enumerate()
        .map(|(i, (_, _, s, _))| s.pp.unwrap_or(0.0) as f64 * 0.95_f64.powi(i as i32))
        .sum::<f64>();
    unchoked_pp = (100.0 * (unchoked_pp + bonus_pp)).round() / 100.0;

    // Accumulate all necessary data
    let pages = numbers::div_euclid(5, scores_data.len());
    let data = NoChokeEmbed::new(&user, scores_data.iter().take(5), unchoked_pp, (1, pages)).await;

    if let Some(msg) = retrieving_msg {
        let _ = ctx.http.delete_message(msg.channel_id, msg.id).await;
    }

    // Creating the embed
    let embed = data.build().build()?;
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .content(format!("No-choke top scores for `{}`:", name))?
        .embed(embed)?
        .await?;

    // Add missing maps to database
    if missing_maps.is_empty() {
        match ctx.psql().insert_beatmaps(&missing_maps).await {
            Ok(n) if n < 2 => {}
            Ok(n) => info!("Added {} maps to DB", n),
            Err(why) => unwind_error!(warn, why, "Error while adding maps to DB: {}"),
        }
    }

    // Skip pagination if too few entries
    if scores_data.len() <= 5 {
        response.reaction_delete(&ctx, msg.author.id);
        return Ok(());
    }

    // Pagination
    let pagination = NoChokePagination::new(response, user, scores_data, unchoked_pp);
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 90).await {
            unwind_error!(warn, why, "Pagination error (nochokes): {}")
        }
    });
    Ok(())
}
