use crate::{
    arguments::TopOldArgs,
    embeds::{EmbedData, TopIfEmbed},
    pagination::{Pagination, TopIfPagination},
    tracking::process_tracking,
    unwind_error,
    util::{
        constants::{DARK_GREEN, OSU_API_ISSUE},
        error::PPError,
        numbers,
        osu::prepare_beatmap_file,
        MessageExt,
    },
    Args, BotResult, Context,
};

use rosu::model::{GameMode, Score};
use rosu_pp::{Beatmap, BeatmapExt};
use rosu_pp_older::*;
use std::{cmp::Ordering, collections::HashMap, fs::File, sync::Arc};
use twilight_embed_builder::builder::EmbedBuilder;
use twilight_model::channel::Message;

macro_rules! pp_std {
    ($version:ident, $map:ident, $rosu_map:ident, $score:ident, $mods:ident, $max_pp:ident) => {{
        let max_pp_result = $version::OsuPP::new(&$rosu_map).mods($mods).calculate();

        $max_pp.replace(max_pp_result.pp());
        $map.stars = max_pp_result.stars();

        let pp_result = $version::OsuPP::new(&$rosu_map)
            .mods($mods)
            .attributes(max_pp_result)
            .n300($score.count300 as usize)
            .n100($score.count100 as usize)
            .n50($score.count50 as usize)
            .misses($score.count_miss as usize)
            .combo($score.max_combo as usize)
            .calculate();

        $score.pp.replace(pp_result.pp());
    }};
}

macro_rules! pp_mna {
    ($version:ident, $map:ident, $rosu_map:ident, $score:ident, $mods:ident, $max_pp:ident) => {{
        let max_pp_result = $version::ManiaPP::new(&$rosu_map).mods($mods).calculate();

        $max_pp.replace(max_pp_result.pp());
        $map.stars = max_pp_result.stars();

        let pp_result = $version::ManiaPP::new(&$rosu_map)
            .mods($mods)
            .attributes(max_pp_result)
            .score($score.score)
            .accuracy($score.accuracy(GameMode::MNA))
            .calculate();

        $score.pp.replace(pp_result.pp());
    }};
}

macro_rules! pp_ctb {
    ($version:ident, $map:ident, $rosu_map:ident, $score:ident, $mods:ident, $max_pp:ident) => {{
        let max_pp_result = $version::FruitsPP::new(&$rosu_map).mods($mods).calculate();

        $max_pp.replace(max_pp_result.pp());
        $map.stars = max_pp_result.stars();

        let pp_result = $version::FruitsPP::new(&$rosu_map)
            .mods($mods)
            .attributes(max_pp_result)
            .fruits($score.count300 as usize)
            .droplets($score.count100 as usize)
            .tiny_droplets($score.count50 as usize)
            .tiny_droplet_misses($score.count_katu as usize)
            .misses($score.count_miss as usize)
            .combo($score.max_combo as usize)
            .calculate();

        $score.pp.replace(pp_result.pp());
    }};
}

macro_rules! pp_tko {
    ($version:ident, $map:ident, $rosu_map:ident, $score:ident, $mods:ident, $max_pp:ident) => {{
        let max_pp_result = $version::TaikoPP::new(&$rosu_map).mods($mods).calculate();

        $max_pp.replace(max_pp_result.pp());
        $map.stars = max_pp_result.stars();

        let pp_result = $version::TaikoPP::new(&$rosu_map)
            .mods($mods)
            .attributes(max_pp_result)
            .n300($score.count300 as usize)
            .n100($score.count100 as usize)
            .misses($score.count_miss as usize)
            .combo($score.max_combo as usize)
            .calculate();

        $score.pp.replace(pp_result.pp());
    }};
}

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

    let content = match (mode, args.year) {
        (GameMode::STD, year) if year < 2007 => Some("osu! was not a thing until september 2007."),
        (GameMode::STD, 2007..=2011) => {
            Some("Up until april 2012, ranked score was the skill metric.")
        }
        (GameMode::STD, 2012..=2013) => Some(
            "April 2012 till january 2014 was the reign of ppv1.\n\
            The source code is not available though \\:(",
        ),
        (GameMode::STD, 2014) => Some(
            "ppv2 replaced ppv1 in january 2014 and lasted until april 2015.\n\
            The source code is not available though \\:(",
        ),
        (GameMode::STD, _) => None,

        (GameMode::MNA, year) if year < 2014 => {
            Some("mania pp were not a thing until march 2014. I think? Don't quote me on that :^)")
        }
        (GameMode::MNA, _) => None,

        (GameMode::TKO, year) if year < 2014 => {
            Some("taiko pp were not a thing until march 2014. I think? Don't quote me on that :^)")
        }
        (GameMode::TKO, _) => None,

        (GameMode::CTB, year) if year < 2014 => {
            Some("ctb pp were not a thing until march 2014. I think? Don't quote me on that :^)")
        }
        (GameMode::CTB, _) => None,
    };

    if let Some(content) = content {
        let embed = EmbedBuilder::new()
            .color(DARK_GREEN)?
            .description(content)?
            .build()?;

        ctx.http
            .create_message(msg.channel_id)
            .embed(embed)?
            .await?
            .reaction_delete(&ctx, msg.author.id);

        return Ok(());
    }

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

        if (mode == GameMode::STD && args.year >= 2021)
            || (mode == GameMode::MNA && args.year >= 2018)
            || (mode == GameMode::TKO && args.year >= 2020)
            || (mode == GameMode::CTB && args.year >= 2020)
        {
            max_pp.replace(rosu_map.max_pp(mods).pp());
            continue;
        }

        match (mode, args.year) {
            (GameMode::STD, 2015..=2017) => pp_std!(osu_2015, map, rosu_map, score, mods, max_pp),
            (GameMode::STD, 2018) => pp_std!(osu_2018, map, rosu_map, score, mods, max_pp),
            (GameMode::STD, 2019..=2020) => pp_std!(osu_2019, map, rosu_map, score, mods, max_pp),

            (GameMode::MNA, 2014..=2017) => pp_mna!(mania_ppv1, map, rosu_map, score, mods, max_pp),

            (GameMode::TKO, 2014..=2019) => pp_tko!(taiko_ppv1, map, rosu_map, score, mods, max_pp),

            (GameMode::CTB, 2014..=2019) => {
                pp_ctb!(fruits_ppv1, map, rosu_map, score, mods, max_pp)
            }

            _ => unreachable!(),
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
        version = content_date_range(mode, args.year),
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
    let post_pp = user.pp_raw;
    let pagination = TopIfPagination::new(response, user, scores_data, mode, adjusted_pp, post_pp);
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (topold): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display a user's top plays on different pp versions")]
#[long_desc(
    "Display how the user's **current** top100 would have looked like \
    in a previous year.\n\
    The osu!standard pp history looks roughly like this:\n  \
    - 2012: ppv1\n  \
    - 2014: ppv2\n  \
    - 2015: High CS nerf(?)\n  \
    - 2018: HD adjustment\n    \
    => https://osu.ppy.sh/home/news/2018-05-16-performance-updates\n  \
    - 2019: Angles, speed, spaced streams\n    \
    => https://osu.ppy.sh/home/news/2019-02-05-new-changes-to-star-rating-performance-points\n  \
    - 2021: Accuracy nerf\n    \
    => https://osu.ppy.sh/home/news/2021-01-14-performance-points-updates"
)]
#[usage("[username] [year]")]
#[example("badewanne3 2018", "\"freddie benson\" 2015")]
#[aliases("to")]
pub async fn topold(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    topold_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's top mania plays on different pp versions")]
#[long_desc(
    "Display how the user's **current** top100 would have looked like \
    in a previous year.\n\
    The osu!mania pp history looks roughly like this:\n  \
    - 2014: ppv1\n  \
    - 2018: ppv2\n    \
    => https://osu.ppy.sh/home/news/2018-05-16-performance-updates"
)]
#[usage("[username] [year]")]
#[example("\"freddie benson\" 2015")]
#[aliases("tom")]
pub async fn topoldmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    topold_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's top taiko plays on different pp versions")]
#[long_desc(
    "Display how the user's **current** top100 would have looked like \
    in a previous year.\n\
    The osu!taiko pp history looks roughly like this:\n  \
    - 2014: ppv1\n  \
    - 2020: Revamp\n    \
    => https://osu.ppy.sh/home/news/2020-09-15-changes-to-osutaiko-star-rating"
)]
#[usage("[username] [year]")]
#[example("\"freddie benson\" 2015")]
#[aliases("tot")]
pub async fn topoldtaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    topold_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's top ctb plays on different pp versions")]
#[long_desc(
    "Display how the user's **current** top100 would have looked like \
    in a previous year.\n\
    The osu!ctb pp history looks roughly like this:\n  \
    - 2014: ppv1\n  \
    - 2020: Revamp\n    \
    => https://osu.ppy.sh/home/news/2020-05-14-osucatch-scoring-updates"
)]
#[usage("[username] [year]")]
#[example("\"freddie benson\" 2019")]
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

fn content_date_range(mode: GameMode, year: u16) -> &'static str {
    match (mode, year) {
        (GameMode::STD, 2007..=2011) => "between 2007 and april 2012",
        (GameMode::STD, 2012..=2013) => "between april 2012 and january 2014",
        (GameMode::STD, 2014) => "between january 2014 and april 2015",
        (GameMode::STD, 2015..=2017) => "between april 2015 and may 2018",
        (GameMode::STD, 2018) => "between may 2018 and february 2019",
        (GameMode::STD, 2019..=2020) => "between february 2019 and january 2021",
        (GameMode::STD, _) => "since january 2021",

        (GameMode::MNA, 2014..=2017) => "between march 2014 and may 2018",
        (GameMode::MNA, _) => "since may 2018",

        (GameMode::TKO, 2014..=2019) => "between march 2014 and september 2020",
        (GameMode::TKO, _) => "since september 2020",

        (GameMode::CTB, 2014..=2020) => "between march 2014 and may 2020",
        (GameMode::CTB, _) => "since may 2020",
    }
}
