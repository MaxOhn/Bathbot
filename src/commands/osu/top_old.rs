use crate::{
    arguments::{PPVersion, TopOldArgs},
    embeds::{EmbedData, TopIfEmbed},
    pagination::{Pagination, TopIfPagination},
    pp::MyOsuPP,
    tracking::process_tracking,
    unwind_error,
    util::{
        constants::OSU_API_ISSUE, error::PPError, numbers, osu::prepare_beatmap_file, MessageExt,
    },
    Args, BotResult, Context,
};

use rosu::model::{GameMode, Score};
use rosu_pp::{Beatmap, BeatmapExt};
use std::{cmp::Ordering, collections::HashMap, fs::File, sync::Arc};
use twilight_model::channel::Message;

async fn topold_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = match TopOldArgs::new(&ctx, args) {
        Ok(args) => args,
        Err(err_msg) => return msg.error(&ctx, err_msg).await,
    };

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

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

        ctx.http
            .create_message(msg.channel_id)
            .content(content)?
            .await
            .ok()
    } else {
        None
    };

    // Calculate bonus pp
    let actual_pp = scores
        .iter()
        .enumerate()
        .map(|(i, Score { pp, .. })| pp.unwrap() as f64 * 0.95_f64.powi(i as i32))
        .sum::<f64>();
    let bonus_pp = user.pp_raw as f64 - actual_pp;

    // Retrieving all missing beatmaps
    let mut scores_data = Vec::with_capacity(scores.len());
    let mut missing_maps = Vec::new();

    for (i, score) in scores.into_iter().enumerate() {
        let map_id = score.beatmap_id.unwrap();

        let map = if let Some(map) = maps.remove(&map_id) {
            map
        } else {
            match ctx.osu().beatmap().map_id(map_id).await {
                Ok(Some(map)) => {
                    missing_maps.push(map.clone());

                    map
                }
                Ok(None) => {
                    let content = format!("The API returned no beatmap for map id {}", map_id);
                    return msg.error(&ctx, content).await;
                }
                Err(why) => {
                    let _ = msg.error(&ctx, OSU_API_ISSUE).await;
                    return Err(why.into());
                }
            }
        };

        scores_data.push((i + 1, score, map, None));
    }

    // Calculate pp values
    for (_, score, map, max_pp) in scores_data.iter_mut() {
        let map_path = prepare_beatmap_file(map.beatmap_id).await?;
        let file = File::open(map_path)?;
        let rosu_map = Beatmap::parse(file).map_err(PPError::from)?;
        let mods = score.enabled_mods.bits();

        if mode != GameMode::STD || args.pp_version == PPVersion::January2021 {
            max_pp.replace(rosu_map.max_pp(mods).pp());
            continue;
        }

        // TODO: Assert valid version-mod combination
        match args.pp_version {
            PPVersion::January2021 => unreachable!(),
            PPVersion::February2019 => {
                let max_pp_result = MyOsuPP::new(&rosu_map).mods(mods).calculate_v2019();

                max_pp.replace(max_pp_result.pp());
                map.stars = max_pp_result.stars();

                let pp_result = MyOsuPP::new(&rosu_map)
                    .mods(mods)
                    .attributes(max_pp_result)
                    .n300(score.count300 as usize)
                    .n100(score.count100 as usize)
                    .n50(score.count50 as usize)
                    .misses(score.count_miss as usize)
                    .combo(score.max_combo as usize)
                    .calculate_v2019();

                score.pp.replace(pp_result.pp());
            }
        }
    }

    // Sort by adjusted pp
    scores_data.sort_unstable_by(|(_, s1, ..), (_, s2, ..)| {
        s2.pp.partial_cmp(&s1.pp).unwrap_or(Ordering::Equal)
    });

    // Calculate adjusted pp
    let adjusted_pp = scores_data
        .iter()
        .map(|(i, Score { pp, .. }, ..)| pp.unwrap_or(0.0) as f64 * 0.95_f64.powi(*i as i32 - 1))
        .sum::<f64>();
    let adjusted_pp = numbers::round((bonus_pp + adjusted_pp).max(0.0) as f32);

    // Accumulate all necessary data
    let content = format!(
        "`{name}`{plural} {mode}top100 {version}:",
        name = user.username,
        plural = plural(user.username.as_str()),
        mode = mode_str(mode),
        version = match args.pp_version {
            PPVersion::January2021 => "since january 2021",
            PPVersion::February2019 => "between february 2019 and january 2021",
        }
    );

    let pages = numbers::div_euclid(5, scores_data.len());

    let data = TopIfEmbed::new(
        &user,
        scores_data.iter().take(5),
        mode,
        adjusted_pp,
        user.pp_raw,
        (1, pages),
    )
    .await;

    if let Some(msg) = retrieving_msg {
        let _ = ctx.http.delete_message(msg.channel_id, msg.id).await;
    }

    // Creating the embed
    let embed = data.build().build()?;
    let create_msg = ctx.http.create_message(msg.channel_id).embed(embed)?;
    let response = create_msg.content(content)?.await?;

    // Add missing maps to database
    if !missing_maps.is_empty() {
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
    let pagination = TopIfPagination::new(response, user, scores_data, mode, adjusted_pp);
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (topold): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display a user's top plays with(out) the given mods")]
// #[long_desc(
//     "Display how a user's top plays would look like with the given mods.\n\
//     As for all other commands with mods input, you can specify them as follows:\n  \
//     - `+mods` to include the mod(s) into all scores\n  \
//     - `+mods!` to make all scores have exactly those mods\n  \
//     - `-mods!` to remove all these mods from all scores"
// )]
// #[usage("[username] [mods]")]
// #[example("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
#[aliases("to")]
pub async fn topold(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    topold_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's top taiko plays with(out) the given mods")]
// #[long_desc(
//     "Display how a user's top taiko plays would look like with the given mods.\n\
//     As for all other commands with mods input, you can specify them as follows:\n  \
//     - `+mods` to include the mod(s) into all scores\n  \
//     - `+mods!` to make all scores have exactly those mods\n  \
//     - `-mods!` to remove all these mods from all scores"
// )]
// #[usage("[username] [mods]")]
// #[example("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
#[aliases("tom")]
pub async fn topoldmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    topold_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's top taiko plays with(out) the given mods")]
// #[long_desc(
//     "Display how a user's top taiko plays would look like with the given mods.\n\
//     As for all other commands with mods input, you can specify them as follows:\n  \
//     - `+mods` to include the mod(s) into all scores\n  \
//     - `+mods!` to make all scores have exactly those mods\n  \
//     - `-mods!` to remove all these mods from all scores"
// )]
// #[usage("[username] [mods]")]
// #[example("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
#[aliases("tot")]
pub async fn topoldtaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    topold_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's top ctb plays with(out) the given mods")]
// #[long_desc(
//     "Display how a user's top ctb plays would look like with the given mods.\n\
//     As for all other commands with mods input, you can specify them as follows:\n  \
//     - `+mods` to include the mod(s) into all scores\n  \
//     - `+mods!` to make all scores have exactly those mods\n  \
//     - `-mods!` to remove all these mods from all scores"
// )]
// #[usage("[username] [mods]")]
// #[example("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
#[aliases("toc")]
pub async fn topoldctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    topold_main(GameMode::CTB, ctx, msg, args).await
}

fn plural(name: &str) -> &'static str {
    match name.chars().last() {
        Some('s') => "'",
        Some(_) | None => "'s",
    }
}

fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::STD => "",
        GameMode::TKO => "taiko ",
        GameMode::CTB => "ctb ",
        GameMode::MNA => "mania ",
    }
}
