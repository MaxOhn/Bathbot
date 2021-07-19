use super::{prepare_scores, request_user, ErrorType};
use crate::{
    arguments::try_link_name,
    embeds::{EmbedData, TopIfEmbed},
    pagination::{Pagination, TopIfPagination},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        error::PPError,
        numbers,
        osu::prepare_beatmap_file,
        CowUtils, MessageExt,
    },
    Args, BotResult, Context,
};

use futures::{
    future::TryFutureExt,
    stream::{FuturesUnordered, TryStreamExt},
};
use rosu_pp_newer::{osu_delta, osu_sotarks, osu_xexxar};
use rosu_v2::prelude::{GameMode, OsuError, Score};
use std::{cmp::Ordering, sync::Arc};
use tokio::fs::File;
use twilight_model::channel::Message;

#[derive(Copy, Clone)]
enum Version {
    Delta,
    Sotarks,
    Xexxar,
}

async fn rebalance_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    mut args: Args<'_>,
) -> BotResult<()> {
    let version = match args.next().map(CowUtils::cow_to_ascii_lowercase).as_deref() {
        Some("xexxar") => Version::Xexxar,
        Some("delta") | Some("delta_t") | Some("deltat") => Version::Delta,
        Some("sotarks") => Version::Sotarks,
        _ => {
            let content = "The first argument must be the version name so either \
            `xexxar`, `delta`, or `sotarks`";

            return msg.error(&ctx, content).await;
        }
    };

    let name = match try_link_name(&ctx, args.next()).or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

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

    // Calculate bonus pp
    let actual_pp: f32 = scores
        .iter()
        .filter_map(|score| score.weight)
        .map(|weight| weight.pp)
        .sum();

    let bonus_pp = user.statistics.as_ref().unwrap().pp - actual_pp;

    let scores_fut = scores
        .into_iter()
        .enumerate()
        .map(|(mut i, mut score)| async move {
            i += 1;
            let map = score.map.as_ref().unwrap();

            if map.convert {
                return Ok((i, score, None));
            }

            // Calculate pp values
            let max_pp = match version {
                Version::Delta => osu_delta(&mut score).await?,
                Version::Sotarks => osu_sotarks(&mut score).await?,
                Version::Xexxar => osu_xexxar(&mut score).await?,
            };

            Ok((i, score, Some(max_pp)))
        })
        .collect::<FuturesUnordered<_>>()
        .try_collect::<Vec<_>>();

    let mut scores_data = match scores_fut.await {
        Ok(scores) => scores,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Sort by adjusted pp
    scores_data.sort_unstable_by(|(_, s1, _), (_, s2, _)| {
        s2.pp.partial_cmp(&s1.pp).unwrap_or(Ordering::Equal)
    });

    // Calculate adjusted pp
    let adjusted_pp: f32 = scores_data
        .iter()
        .enumerate()
        .map(|(i, (_, Score { pp, .. }, ..))| pp.unwrap_or(0.0) * 0.95_f32.powi(i as i32))
        .sum();

    let post_pp = numbers::round((bonus_pp + adjusted_pp).max(0.0) as f32);

    // Accumulate all necessary data
    let content = format!(
        "`{name}`{plural} {mode}top100 {version}:",
        name = user.username,
        plural = plural(user.username.as_str()),
        mode = mode_str(mode),
        version = content_version(version),
    );

    let pages = numbers::div_euclid(5, scores_data.len());
    let pre_pp = user.statistics.as_ref().unwrap().pp;

    let data = TopIfEmbed::new(
        &user,
        scores_data.iter().take(5),
        mode,
        pre_pp,
        post_pp,
        (1, pages),
    )
    .await;

    // Creating the embed
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .embed(data.into_builder().build())?
        .content(content)?
        .await?;

    // Don't add maps of scores to DB since their stars were potentially changed

    // Skip pagination if too few entries
    if scores_data.len() <= 5 {
        response.reaction_delete(&ctx, msg.author.id);

        return Ok(());
    }

    // Pagination
    let pagination = TopIfPagination::new(response, user, scores_data, mode, pre_pp, post_pp);
    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (rebalance): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display a user's top plays on \"upcoming\" pp versions")]
#[long_desc(
    "Display how the user's **current** top100 would look like \
    in an alternative new pp version.\n\
    Note that the command will **not** change scores, just recalculate their pp.\n\
    To use this command, specify the version name **first**, then a username.\n\
    Available versions are:\n  \
    - `xexxar` (see https://github.com/emu1337/osu) [commit 75735d2]\n  \
    - `delta` (see https://github.com/HeBuwei/osu) [commit 422d74e]\n  \
    - `sotarks` (see https://sotarks.stanr.info/)\n\
    The translations are not exactly accurate so expect a few differences in the results.
    There are also no guarantees that the implemented versions are up-to-date."
)]
#[usage("[version name] [username]")]
#[example("xexxar badewanne3", "delta \"freddie benson\"", "sotarks peppy")]
pub async fn rebalance(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    rebalance_main(GameMode::STD, ctx, msg, args).await
}

async fn osu_xexxar(score: &mut Score) -> BotResult<f32> {
    let map_path = prepare_beatmap_file(score.map.as_ref().unwrap().map_id).await?;
    let file = File::open(map_path).await.map_err(PPError::from)?;
    let rosu_map = osu_xexxar::Beatmap::parse(file)
        .await
        .map_err(PPError::from)?;
    let mods = score.mods.bits();

    let max_pp_result = osu_xexxar::OsuPP::new(&rosu_map).mods(mods).calculate();

    let max_pp = max_pp_result.pp();
    score.map.as_mut().unwrap().stars = max_pp_result.stars();

    let pp_result = osu_xexxar::OsuPP::new(&rosu_map)
        .mods(mods)
        .attributes(max_pp_result)
        .n300(score.statistics.count_300 as usize)
        .n100(score.statistics.count_100 as usize)
        .n50(score.statistics.count_50 as usize)
        .misses(score.statistics.count_miss as usize)
        .combo(score.max_combo as usize)
        .calculate();

    score.pp.replace(pp_result.pp());

    Ok(max_pp)
}

async fn osu_delta(score: &mut Score) -> BotResult<f32> {
    let map_path = prepare_beatmap_file(score.map.as_ref().unwrap().map_id).await?;
    let file = File::open(map_path).await.map_err(PPError::from)?;
    let rosu_map = rosu_pp::Beatmap::parse(file).await.map_err(PPError::from)?;
    let mods = score.mods.bits();

    let max_pp_result = osu_delta::OsuPP::new(&rosu_map).mods(mods).calculate();

    let max_pp = max_pp_result.pp();
    score.map.as_mut().unwrap().stars = max_pp_result.stars();

    let pp_result = osu_delta::OsuPP::new(&rosu_map)
        .mods(mods)
        .attributes(max_pp_result)
        .n300(score.statistics.count_300 as usize)
        .n100(score.statistics.count_100 as usize)
        .n50(score.statistics.count_50 as usize)
        .misses(score.statistics.count_miss as usize)
        .combo(score.max_combo as usize)
        .calculate();

    score.pp.replace(pp_result.pp());

    Ok(max_pp)
}

async fn osu_sotarks(score: &mut Score) -> BotResult<f32> {
    let map_path = prepare_beatmap_file(score.map.as_ref().unwrap().map_id).await?;
    let file = File::open(map_path).await.map_err(PPError::from)?;
    let rosu_map = osu_sotarks::Beatmap::parse(file)
        .await
        .map_err(PPError::from)?;
    let mods = score.mods.bits();

    let max_pp_result = osu_sotarks::OsuPP::new(&rosu_map).mods(mods).calculate();

    let max_pp = max_pp_result.pp();
    score.map.as_mut().unwrap().stars = max_pp_result.stars();

    let pp_result = osu_sotarks::OsuPP::new(&rosu_map)
        .mods(mods)
        .attributes(max_pp_result)
        .n300(score.statistics.count_300 as usize)
        .n100(score.statistics.count_100 as usize)
        .n50(score.statistics.count_50 as usize)
        .misses(score.statistics.count_miss as usize)
        .combo(score.max_combo as usize)
        .calculate();

    score.pp.replace(pp_result.pp());

    Ok(max_pp)
}

#[inline]
fn plural(name: &str) -> &'static str {
    match name.chars().last() {
        Some('s') => "'",
        Some(_) | None => "'s",
    }
}

#[inline]
fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::STD => "",
        GameMode::TKO => "taiko ",
        GameMode::CTB => "ctb ",
        GameMode::MNA => "mania ",
    }
}

#[inline]
fn content_version(version: Version) -> &'static str {
    match version {
        Version::Delta => "on the delta_t version",
        Version::Sotarks => "on the Sotarks rebalance",
        Version::Xexxar => "on the Xexxar version",
    }
}
