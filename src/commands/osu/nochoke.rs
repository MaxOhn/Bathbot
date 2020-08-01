use super::require_link;
use crate::{
    arguments::NameIntArgs,
    bail,
    embeds::{EmbedData, NoChokeEmbed},
    pagination::{NoChokePagination, Pagination},
    pp::{Calculations, PPCalculator},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        numbers, osu, MessageExt,
    },
    Args, BotResult, Context, Error,
};

use futures::future::try_join_all;
use rosu::{backend::requests::BestRequest, models::GameMode};
use std::{cmp::Ordering, collections::HashMap, sync::Arc};
use twilight::model::channel::Message;

#[command]
#[short_desc("Unchoke a user's top100")]
#[long_desc(
    "Display a user's top plays if no score in their top 100 \
     would be a choke.\nIf a number is specified, \
     I will only unchoke scores with at most that many misses"
)]
#[usage("[username] [number for miss limit]")]
#[example("badewanne3")]
#[example("vaxei 5")]
#[aliases("nc", "nochoke")]
async fn nochokes(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = NameIntArgs::new(&ctx, args);
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return require_link(&ctx, msg).await,
    };
    let miss_limit = args.number;

    // Retrieve the user and their top scores
    let scores_fut = BestRequest::with_username(&name)
        .mode(GameMode::STD)
        .limit(100)
        .queue(ctx.osu());
    let join_result = tokio::try_join!(ctx.osu_user(&name, GameMode::STD), scores_fut);
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

    // Get all relevant maps from the database
    let map_ids: Vec<u32> = scores.iter().filter_map(|s| s.beatmap_id).collect();
    let mut maps = match ctx.psql().get_beatmaps(&map_ids).await {
        Ok(maps) => maps,
        Err(why) => {
            warn!("Error while getting maps from DB: {}", why);
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
        let map = if maps.contains_key(&map_id) {
            maps.remove(&map_id).unwrap()
        } else {
            let map = match score.get_beatmap(ctx.osu()).await {
                Ok(map) => map,
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
        if score.max_combo != map.max_combo.unwrap()
            && (miss_limit.is_none() || score.count_miss <= miss_limit.unwrap())
        {
            osu::unchoke_score(&mut unchoked, &map);
            let mut calculator = PPCalculator::new().score(&unchoked).map(&map);
            calculator.calculate(Calculations::PP, None).await?;
            unchoked.pp = calculator.pp();
        }
        Ok::<_, Error>((i, score, unchoked, map))
    });
    let mut scores_data = match try_join_all(unchoke_fut).await {
        Ok(scores_data) => scores_data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            bail!("error while unchoking scores: {}", why);
        }
    };

    // Sort by unchoked pp
    scores_data.sort_unstable_by(|(_, _, s1, _), (_, _, s2, _)| {
        s2.pp.partial_cmp(&s1.pp).unwrap_or(Ordering::Equal)
    });

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
    let data =
        match NoChokeEmbed::new(&user, scores_data.iter().take(5), unchoked_pp, (1, pages)).await {
            Ok(data) => data,
            Err(why) => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;
                bail!("error while creating embed: {}", why);
            }
        };

    if let Some(msg) = retrieving_msg {
        let _ = ctx.http.delete_message(msg.channel_id, msg.id).await;
    }

    // Creating the embed
    let embed = data.build().build();
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
            Err(why) => warn!("Error while adding maps to DB: {}", why),
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
            warn!("Pagination error: {}", why)
        }
    });
    Ok(())
}
