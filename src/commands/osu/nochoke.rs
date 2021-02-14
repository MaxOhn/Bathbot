use crate::{
    arguments::NameIntArgs,
    bail,
    embeds::{EmbedData, NoChokeEmbed},
    pagination::{NoChokePagination, Pagination},
    tracking::process_tracking,
    unwind_error,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        error::PPError,
        numbers,
        osu::prepare_beatmap_file,
        MessageExt,
    },
    Args, BotResult, Context, Error,
};

use futures::future::try_join_all;
use rosu::model::GameMode;
use rosu_pp::{Beatmap as Map, FruitsPP, OsuPP, StarResult, TaikoPP};
use std::{cmp::Ordering, collections::HashMap, fs::File, sync::Arc};
use twilight_model::channel::Message;

async fn nochokes_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = NameIntArgs::new(&ctx, args);

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    let miss_limit = args.number;

    // Retrieve the user and their top scores
    let user_fut = ctx.osu().user(name.as_str()).mode(mode);
    let scores_fut = ctx.osu().top_scores(name.as_str()).mode(mode).limit(100);
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
    process_tracking(&ctx, mode, &scores, Some(&user), &mut maps).await;

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

        // Skip unchoking because it has too many misses or because its a convert
        if miss_limit
            .filter(|&limit| score.count_miss > limit)
            .is_some()
            || mode != map.mode
        {
            return Ok((i, score, unchoked, map));
        }

        let map_path = prepare_beatmap_file(map.beatmap_id).await?;
        let file = File::open(map_path).map_err(PPError::from)?;
        let rosu_map = Map::parse(file).map_err(PPError::from)?;
        let mods = score.enabled_mods.bits();

        match map.mode {
            GameMode::STD
                if score.count_miss > 0
                    || score.max_combo < map.max_combo.unwrap_or(5).saturating_sub(5) =>
            {
                let total_objects =
                    (map.count_circle + map.count_slider + map.count_spinner) as usize;

                let mut count300 = score.count300 as usize;

                let count_hits = total_objects - score.count_miss as usize;
                let ratio = 1.0 - (count300 as f32 / count_hits as f32);
                let new100s = (ratio * score.count_miss as f32).ceil() as u32;

                count300 += score.count_miss.saturating_sub(new100s) as usize;
                let count100 = (score.count100 + new100s) as usize;
                let count50 = score.count50 as usize;

                let pp_result = OsuPP::new(&rosu_map)
                    .mods(mods)
                    .n300(count300)
                    .n100(count100)
                    .n50(count50)
                    .calculate();

                unchoked.count300 = count300 as u32;
                unchoked.count100 = count100 as u32;
                unchoked.max_combo = map.max_combo.unwrap_or(0);
                unchoked.count_miss = 0;
                unchoked.pp = Some(pp_result.pp);
                unchoked.recalculate_grade(map.mode, None);
            }
            GameMode::CTB if score.max_combo != map.max_combo.unwrap_or(0) => {
                let attributes = match rosu_pp::fruits::stars(&rosu_map, mods, None) {
                    StarResult::Fruits(attributes) => attributes,
                    _ => bail!("no ctb attributes after calculating stars for ctb map"),
                };

                let total_objects = attributes.max_combo;
                let passed_objects = (score.count300 + score.count100 + score.count_miss) as usize;

                let missing = total_objects.saturating_sub(passed_objects);
                let missing_fruits = missing.saturating_sub(
                    attributes
                        .n_droplets
                        .saturating_sub(score.count100 as usize),
                );
                let missing_droplets = missing - missing_fruits;

                let n_fruits = score.count300 as usize + missing_fruits;
                let n_droplets = score.count100 as usize + missing_droplets;
                let n_tiny_droplet_misses = score.count_katu as usize;
                let n_tiny_droplets = score.count50 as usize;

                let pp_result = FruitsPP::new(&rosu_map)
                    .attributes(attributes)
                    .mods(mods)
                    .fruits(n_fruits)
                    .droplets(n_droplets)
                    .tiny_droplets(n_tiny_droplets)
                    .tiny_droplet_misses(n_tiny_droplet_misses)
                    .calculate();

                let hits = n_fruits + n_droplets + n_tiny_droplets;
                let total = hits + n_tiny_droplet_misses;

                let acc = if total == 0 {
                    0.0
                } else {
                    100.0 * hits as f32 / total as f32
                };

                unchoked.count300 = n_fruits as u32;
                unchoked.count_katu = n_tiny_droplet_misses as u32;
                unchoked.count100 = n_droplets as u32;
                unchoked.count50 = n_tiny_droplets as u32;
                unchoked.max_combo = total_objects as u32;
                unchoked.count_miss = 0;
                unchoked.pp = Some(pp_result.pp);
                unchoked.recalculate_grade(map.mode, Some(acc));
            }
            GameMode::TKO if score.count_miss > 0 => {
                let total_objects = map.count_circle as usize;
                let passed_objects = score.total_hits(GameMode::TKO) as usize;

                let mut count300 =
                    score.count300 as usize + total_objects.saturating_sub(passed_objects);

                let count_hits = total_objects - score.count_miss as usize;
                let ratio = 1.0 - (count300 as f32 / count_hits as f32);
                let new100s = (ratio * score.count_miss as f32).ceil() as u32;

                count300 += score.count_miss.saturating_sub(new100s) as usize;
                let count100 = (score.count100 + new100s) as usize;

                let acc = 100.0 * (2 * count300 + count100) as f32 / (2 * total_objects) as f32;

                let pp_result = TaikoPP::new(&rosu_map).mods(mods).accuracy(acc).calculate();

                unchoked.count300 = count300 as u32;
                unchoked.count100 = count100 as u32;
                unchoked.count_miss = 0;
                unchoked.pp = Some(pp_result.pp);
                unchoked.recalculate_grade(map.mode, Some(acc));
            }
            GameMode::MNA => bail!("can not unchoke mania scores"),
            _ => {} // Nothing to unchoke
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
        .content(format!(
            "No-choke top {}scores for `{}`:",
            match mode {
                GameMode::STD => "",
                GameMode::TKO => "taiko ",
                GameMode::CTB => "ctb ",
                GameMode::MNA => panic!("can not unchoke mania scores"),
            },
            name
        ))?
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

#[command]
#[short_desc("Unchoke a user's top100")]
#[long_desc(
    "Display a user's top plays if no score in their top100 would be a choke.\n
    If a number is specified, I will only unchoke scores with at most that many misses"
)]
#[usage("[username] [number for miss limit]")]
#[example("badewanne3", "vaxei 5")]
#[aliases("nc", "nochoke")]
async fn nochokes(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    nochokes_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Unchoke a user's taiko top100")]
#[long_desc(
    "Display a user's top plays if no score in their top100 would be a choke.\n\
    If a number is specified, I will only unchoke scores with at most that many misses.\n\
    Note: As for all commands, numbers for scores on converted maps are wack and \
    are ignored when unchoking."
)]
#[usage("[username] [number for miss limit]")]
#[example("badewanne3", "vaxei 5")]
#[aliases("nct", "nochoketaiko")]
async fn nochokestaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    nochokes_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Unchoke a user's ctb top100")]
#[long_desc(
    "Display a user's top plays if no score in their top100 would be a choke.\n\
    If a number is specified, I will only unchoke scores with at most that many misses.\n\
    Note: As for all commands, numbers for scores on converted maps are wack and \
    are ignored when unchoking."
)]
#[usage("[username] [number for miss limit]")]
#[example("badewanne3", "vaxei 5")]
#[aliases("ncc", "nochokectb")]
async fn nochokesctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    nochokes_main(GameMode::CTB, ctx, msg, args).await
}
