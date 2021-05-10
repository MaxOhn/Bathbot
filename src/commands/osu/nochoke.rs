use super::{prepare_scores, request_user, ErrorType};
use crate::{
    arguments::NameIntArgs,
    bail,
    embeds::{EmbedData, NoChokeEmbed},
    pagination::{NoChokePagination, Pagination},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        error::PPError,
        numbers,
        osu::prepare_beatmap_file,
        MessageExt,
    },
    Args, BotResult, Context, Error,
};

use futures::{
    future::TryFutureExt,
    stream::{FuturesUnordered, TryStreamExt},
};
use rosu_pp::{Beatmap as Map, FruitsPP, OsuPP, StarResult, TaikoPP};
use rosu_v2::prelude::{GameMode, OsuError};
use std::{cmp::Ordering, sync::Arc};
use tokio::fs::File;
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
    let user_fut = request_user(&ctx, &name, Some(mode)).map_err(From::from);
    let scores_fut_1 = ctx
        .osu()
        .user_scores(name.as_str())
        .best()
        .mode(mode)
        .limit(50);

    let scores_fut_2 = ctx
        .osu()
        .user_scores(name.as_str())
        .best()
        .mode(mode)
        .offset(50)
        .limit(50);

    let scores_fut_1 = prepare_scores(&ctx, scores_fut_1);
    let scores_fut_2 = prepare_scores(&ctx, scores_fut_2);

    let (user, mut scores) = match tokio::try_join!(user_fut, scores_fut_1, scores_fut_2) {
        Ok((user, mut scores, mut scores_2)) => {
            scores.append(&mut scores_2);

            (user, scores)
        }
        Err(ErrorType::Osu(OsuError::NotFound)) => {
            let content = format!("User `{}` was not found", name);

            return msg.error(&ctx, content).await;
        }
        Err(ErrorType::Osu(why)) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
        Err(ErrorType::Bot(why)) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Process user and their top scores for tracking
    process_tracking(&ctx, mode, &mut scores, Some(&user)).await;

    // Unchoke scores asynchronously
    let unchoke_fut = scores
        .into_iter()
        .enumerate()
        .map(|(mut i, score)| async move {
            i += 1;
            let map = score.map.as_ref().unwrap();
            let mut unchoked = score.clone();

            let many_misses = miss_limit
                .filter(|&limit| score.statistics.count_miss > limit)
                .is_some();

            // Skip unchoking because it has too many misses or because its a convert
            if many_misses || map.convert {
                return Ok((i, score, unchoked));
            }

            let map_path = prepare_beatmap_file(map.map_id).await?;
            let file = File::open(map_path).await.map_err(PPError::from)?;
            let rosu_map = Map::parse(file).await.map_err(PPError::from)?;
            let mods = score.mods.bits();

            match map.mode {
                GameMode::STD
                    if score.statistics.count_miss > 0
                        || score.max_combo < map.max_combo.unwrap_or(5).saturating_sub(5) =>
                {
                    let total_objects = map.count_objects() as usize;

                    let mut count300 = score.statistics.count_300 as usize;

                    let count_hits = total_objects - score.statistics.count_miss as usize;
                    let ratio = 1.0 - (count300 as f32 / count_hits as f32);
                    let new100s = (ratio * score.statistics.count_miss as f32).ceil() as u32;

                    count300 += score.statistics.count_miss.saturating_sub(new100s) as usize;
                    let count100 = (score.statistics.count_100 + new100s) as usize;
                    let count50 = score.statistics.count_50 as usize;

                    let pp_result = OsuPP::new(&rosu_map)
                        .mods(mods)
                        .n300(count300)
                        .n100(count100)
                        .n50(count50)
                        .calculate();

                    unchoked.statistics.count_300 = count300 as u32;
                    unchoked.statistics.count_100 = count100 as u32;
                    unchoked.max_combo = map.max_combo.unwrap_or(0);
                    unchoked.statistics.count_miss = 0;
                    unchoked.pp = Some(pp_result.pp);
                    unchoked.grade = unchoked.grade(None);
                    unchoked.accuracy = unchoked.accuracy();
                }
                GameMode::CTB if score.max_combo != map.max_combo.unwrap_or(0) => {
                    let attributes = match rosu_pp::fruits::stars(&rosu_map, mods, None) {
                        StarResult::Fruits(attributes) => attributes,
                        _ => bail!("no ctb attributes after calculating stars for ctb map"),
                    };

                    let total_objects = attributes.max_combo;
                    let passed_objects = (score.statistics.count_300
                        + score.statistics.count_100
                        + score.statistics.count_miss)
                        as usize;

                    let missing = total_objects.saturating_sub(passed_objects);
                    let missing_fruits = missing.saturating_sub(
                        attributes
                            .n_droplets
                            .saturating_sub(score.statistics.count_100 as usize),
                    );
                    let missing_droplets = missing - missing_fruits;

                    let n_fruits = score.statistics.count_300 as usize + missing_fruits;
                    let n_droplets = score.statistics.count_100 as usize + missing_droplets;
                    let n_tiny_droplet_misses = score.statistics.count_katu as usize;
                    let n_tiny_droplets = score.statistics.count_50 as usize;

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

                    unchoked.statistics.count_300 = n_fruits as u32;
                    unchoked.statistics.count_katu = n_tiny_droplet_misses as u32;
                    unchoked.statistics.count_100 = n_droplets as u32;
                    unchoked.statistics.count_50 = n_tiny_droplets as u32;
                    unchoked.max_combo = total_objects as u32;
                    unchoked.statistics.count_miss = 0;
                    unchoked.pp = Some(pp_result.pp);
                    unchoked.grade = unchoked.grade(Some(acc));
                    unchoked.accuracy = unchoked.accuracy();
                }
                GameMode::TKO if score.statistics.count_miss > 0 => {
                    let total_objects = map.count_circles as usize;
                    let passed_objects = score.total_hits() as usize;

                    let mut count300 = score.statistics.count_300 as usize
                        + total_objects.saturating_sub(passed_objects);

                    let count_hits = total_objects - score.statistics.count_miss as usize;
                    let ratio = 1.0 - (count300 as f32 / count_hits as f32);
                    let new100s = (ratio * score.statistics.count_miss as f32).ceil() as u32;

                    count300 += score.statistics.count_miss.saturating_sub(new100s) as usize;
                    let count100 = (score.statistics.count_100 + new100s) as usize;

                    let acc = 100.0 * (2 * count300 + count100) as f32 / (2 * total_objects) as f32;

                    let pp_result = TaikoPP::new(&rosu_map).mods(mods).accuracy(acc).calculate();

                    unchoked.statistics.count_300 = count300 as u32;
                    unchoked.statistics.count_100 = count100 as u32;
                    unchoked.statistics.count_miss = 0;
                    unchoked.pp = Some(pp_result.pp);
                    unchoked.grade = unchoked.grade(Some(acc));
                    unchoked.accuracy = unchoked.accuracy();
                }
                GameMode::MNA => bail!("can not unchoke mania scores"),
                _ => {} // Nothing to unchoke
            }

            Ok::<_, Error>((i, score, unchoked))
        })
        .collect::<FuturesUnordered<_>>()
        .try_collect();

    let mut scores_data: Vec<_> = match unchoke_fut.await {
        Ok(scores_data) => scores_data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Calculate bonus pp
    let actual_pp: f32 = scores_data
        .iter()
        .filter_map(|(_, s, ..)| s.weight)
        .map(|weight| weight.pp)
        .sum();

    let bonus_pp = user.statistics.as_ref().unwrap().pp - actual_pp;

    // Sort by unchoked pp
    scores_data.sort_unstable_by(|(_, _, s1), (_, _, s2)| {
        s2.pp.partial_cmp(&s1.pp).unwrap_or(Ordering::Equal)
    });

    // Calculate total user pp without chokes
    let mut unchoked_pp: f32 = scores_data
        .iter()
        .enumerate()
        .map(|(i, (_, _, s))| s.pp.unwrap_or(0.0) * 0.95_f32.powi(i as i32))
        .sum();

    unchoked_pp = (100.0 * (unchoked_pp + bonus_pp)).round() / 100.0;

    // Accumulate all necessary data
    let pages = numbers::div_euclid(5, scores_data.len());
    let data = NoChokeEmbed::new(&user, scores_data.iter().take(5), unchoked_pp, (1, pages)).await;

    // Creating the embed
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
        .embed(data.into_builder().build())?
        .await?;

    // Add maps of scores to DB
    let scores_iter = scores_data.iter().map(|(_, score, _)| score);

    if let Err(why) = ctx.psql().store_scores_maps(scores_iter).await {
        unwind_error!(warn, why, "Error while adding score maps to DB: {}")
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
