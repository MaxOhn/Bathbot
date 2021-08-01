use super::{prepare_scores, request_user, ErrorType};
use crate::{
    arguments::NameIntArgs,
    embeds::{EmbedData, TopIfEmbed},
    pagination::{Pagination, TopIfPagination},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        error::PPError,
        numbers,
        osu::prepare_beatmap_file,
        MessageExt,
    },
    Args, BotResult, Context,
};

use chrono::{Datelike, Utc};
use futures::{
    future::TryFutureExt,
    stream::{FuturesUnordered, TryStreamExt},
};
use rosu_pp::{Beatmap, BeatmapExt};
use rosu_pp_older::*;
use rosu_v2::prelude::{GameMode, OsuError, Score};
use std::{cmp::Ordering, sync::Arc};
use tokio::fs::File;
use twilight_model::channel::Message;

macro_rules! pp_std {
    ($version:ident, $rosu_map:ident, $score:ident, $mods:ident) => {{
        let max_pp_result = $version::OsuPP::new(&$rosu_map).mods($mods).calculate();

        let max_pp = max_pp_result.pp();
        $score.map.as_mut().unwrap().stars = max_pp_result.stars();

        let pp_result = $version::OsuPP::new(&$rosu_map)
            .mods($mods)
            .attributes(max_pp_result)
            .n300($score.statistics.count_300 as usize)
            .n100($score.statistics.count_100 as usize)
            .n50($score.statistics.count_50 as usize)
            .misses($score.statistics.count_miss as usize)
            .combo($score.max_combo as usize)
            .calculate();

        $score.pp.replace(pp_result.pp());

        max_pp
    }};
}

macro_rules! pp_mna {
    ($version:ident, $rosu_map:ident, $score:ident, $mods:ident) => {{
        let max_pp_result = $version::ManiaPP::new(&$rosu_map).mods($mods).calculate();

        let max_pp = max_pp_result.pp();
        $score.map.as_mut().unwrap().stars = max_pp_result.stars();

        let pp_result = $version::ManiaPP::new(&$rosu_map)
            .mods($mods)
            .attributes(max_pp_result)
            .score($score.score)
            .accuracy($score.accuracy)
            .calculate();

        $score.pp.replace(pp_result.pp());

        max_pp
    }};
}

macro_rules! pp_ctb {
    ($version:ident, $rosu_map:ident, $score:ident, $mods:ident) => {{
        let max_pp_result = $version::FruitsPP::new(&$rosu_map).mods($mods).calculate();

        let max_pp = max_pp_result.pp();
        $score.map.as_mut().unwrap().stars = max_pp_result.stars();

        let pp_result = $version::FruitsPP::new(&$rosu_map)
            .mods($mods)
            .attributes(max_pp_result)
            .fruits($score.statistics.count_300 as usize)
            .droplets($score.statistics.count_100 as usize)
            .tiny_droplets($score.statistics.count_50 as usize)
            .tiny_droplet_misses($score.statistics.count_katu as usize)
            .misses($score.statistics.count_miss as usize)
            .combo($score.max_combo as usize)
            .calculate();

        $score.pp.replace(pp_result.pp());

        max_pp
    }};
}

macro_rules! pp_tko {
    ($version:ident, $rosu_map:ident, $score:ident, $mods:ident) => {{
        let max_pp_result = $version::TaikoPP::new(&$rosu_map).mods($mods).calculate();

        let max_pp = max_pp_result.pp();
        $score.map.as_mut().unwrap().stars = max_pp_result.stars();

        let pp_result = $version::TaikoPP::new(&$rosu_map)
            .mods($mods)
            .attributes(max_pp_result)
            .n300($score.statistics.count_300 as usize)
            .n100($score.statistics.count_100 as usize)
            .misses($score.statistics.count_miss as usize)
            .combo($score.max_combo as usize)
            .calculate();

        $score.pp.replace(pp_result.pp());

        max_pp
    }};
}

async fn topold_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = NameIntArgs::new(&ctx, args);

    let year = args.number.unwrap_or_else(|| Utc::now().year() as u32);

    let content = match (mode, year) {
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
        msg.send_response(&ctx, content).await?;

        return Ok(());
    }

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    // Retrieve the user and their top scores
    let user_fut = request_user(&ctx, &name, Some(mode)).map_err(From::from);
    let scores_fut = ctx
        .osu()
        .user_scores(name.as_str())
        .best()
        .mode(mode)
        .limit(100);

    let scores_fut = prepare_scores(&ctx, scores_fut);

    let (user, mut scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((user, scores)) => (user, scores),
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

            let map_path = prepare_beatmap_file(map.map_id).await?;
            let file = File::open(map_path).await.map_err(PPError::from)?;
            let rosu_map = Beatmap::parse(file).await.map_err(PPError::from)?;
            let mods = score.mods.bits();

            if (mode == GameMode::STD && year >= 2022)
                || (mode == GameMode::MNA && year >= 2018)
                || (mode == GameMode::TKO && year >= 2020)
                || (mode == GameMode::CTB && year >= 2020)
            {
                return Ok((i, score, Some(rosu_map.max_pp(mods).pp())));
            }

            // Calculate pp values
            let max_pp = match (mode, year) {
                (GameMode::STD, 2015..=2017) => pp_std!(osu_2015, rosu_map, score, mods),
                (GameMode::STD, 2018) => pp_std!(osu_2018, rosu_map, score, mods),
                (GameMode::STD, 2019..=2020) => pp_std!(osu_2019, rosu_map, score, mods),
                (GameMode::STD, 2021..=2021) => pp_std!(osu_2021, rosu_map, score, mods),
                (GameMode::MNA, 2014..=2017) => pp_mna!(mania_ppv1, rosu_map, score, mods),
                (GameMode::TKO, 2014..=2019) => pp_tko!(taiko_ppv1, rosu_map, score, mods),
                (GameMode::CTB, 2014..=2019) => pp_ctb!(fruits_ppv1, rosu_map, score, mods),
                _ => unreachable!(),
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
        .map(|(i, Score { pp, .. }, ..)| pp.unwrap_or(0.0) * 0.95_f32.powi(*i as i32 - 1))
        .sum();

    let adjusted_pp = numbers::round((bonus_pp + adjusted_pp).max(0.0) as f32);

    // Accumulate all necessary data
    let content = format!(
        "`{name}`{plural} {mode}top100 {version}:",
        name = user.username,
        plural = plural(user.username.as_str()),
        mode = mode_str(mode),
        version = content_date_range(mode, year),
    );

    let pages = numbers::div_euclid(5, scores_data.len());
    let post_pp = user.statistics.as_ref().unwrap().pp;

    let data = TopIfEmbed::new(
        &user,
        scores_data.iter().take(5),
        mode,
        adjusted_pp,
        post_pp,
        (1, pages),
    )
    .await;

    // Creating the embed
    let embed = &[data.into_builder().build()];

    let response = msg
        .build_response_msg(&ctx, |m| m.content(&content)?.embeds(embed))
        .await?;

    // * Don't add maps of scores to DB since their stars were potentially changed

    // Skip pagination if too few entries
    if scores_data.len() <= 5 {
        response.reaction_delete(&ctx, msg.author.id);

        return Ok(());
    }

    // Pagination
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
    Note that the command will **not** change scores, just recalculate their pp.\n\
    The osu!standard pp history looks roughly like this:\n  \
    - 2012: ppv1 (unavailable)\n  \
    - 2014: ppv2 (unavailable)\n  \
    - 2015: High CS nerf(?)\n  \
    - 2018: HD adjustment\n    \
    => https://osu.ppy.sh/home/news/2018-05-16-performance-updates\n  \
    - 2019: Angles, speed, spaced streams\n    \
    => https://osu.ppy.sh/home/news/2019-02-05-new-changes-to-star-rating-performance-points\n  \
    - 2021: High AR nerf, NF & SO buff, speed & acc adjustment\n    \
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
    Note that the command will **not** change scores, just recalculate their pp.\n\
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
    Note that the command will **not** change scores, just recalculate their pp.\n\
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
    Note that the command will **not** change scores, just recalculate their pp.\n\
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

fn content_date_range(mode: GameMode, year: u32) -> &'static str {
    match (mode, year) {
        (GameMode::STD, 2007..=2011) => "between 2007 and april 2012",
        (GameMode::STD, 2012..=2013) => "between april 2012 and january 2014",
        (GameMode::STD, 2014) => "between january 2014 and april 2015",
        (GameMode::STD, 2015..=2017) => "between april 2015 and may 2018",
        (GameMode::STD, 2018) => "between may 2018 and february 2019",
        (GameMode::STD, 2019..=2020) => "between february 2019 and january 2021",
        (GameMode::STD, 2021..=2021) => "between january 2021 and july 2021",
        (GameMode::STD, _) => "since july 2021",

        (GameMode::MNA, 2014..=2017) => "between march 2014 and may 2018",
        (GameMode::MNA, _) => "since may 2018",

        (GameMode::TKO, 2014..=2019) => "between march 2014 and september 2020",
        (GameMode::TKO, _) => "since september 2020",

        (GameMode::CTB, 2014..=2020) => "between march 2014 and may 2020",
        (GameMode::CTB, _) => "since may 2020",
    }
}
