use crate::{
    arguments::{Args, TopArgs},
    embeds::{EmbedData, TopEmbed},
    pagination::{Pagination, TopPagination},
    tracking::process_tracking,
    unwind_error,
    util::{constants::OSU_API_ISSUE, numbers, osu::ModSelection, MessageExt},
    BotResult, Context,
};

use rosu::model::{GameMode, Score};
use std::{cmp::Ordering, collections::HashMap, sync::Arc};
use twilight_model::channel::Message;

async fn top_main(
    mode: GameMode,
    top_type: TopType,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let mut args = match TopArgs::new(&ctx, args) {
        Ok(args) => args,
        Err(err_msg) => return msg.error(&ctx, err_msg).await,
    };
    if top_type == TopType::Top && (args.has_dash_r || args.has_dash_p) {
        let mode_long = match mode {
            GameMode::STD => "",
            GameMode::MNA => "mania",
            GameMode::TKO => "taiko",
            GameMode::CTB => "ctb",
        };
        let prefix = ctx.config_first_prefix(msg.guild_id);
        if args.has_dash_r {
            let mode_short = match mode {
                GameMode::STD => "",
                GameMode::MNA => "m",
                GameMode::TKO => "t",
                GameMode::CTB => "c",
            };
            let content = format!(
                "`{prefix}top{mode_long} -r`? I think you meant `{prefix}recentbest{mode_long}` \
                or `{prefix}rb{mode_short}` for short ;)",
                mode_long = mode_long,
                mode_short = mode_short,
                prefix = prefix
            );
            return msg.error(&ctx, content).await;
        } else if args.has_dash_p {
            let content = format!(
                "`{prefix}top{} -p`? Try using the arrow reactions instead ;)",
                mode = mode_long,
                prefix = prefix
            );
            return msg.error(&ctx, content).await;
        }
    }
    let name = match args.name.take().or_else(|| ctx.get_link(msg.author.id.0)) {
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

    // Filter scores according to mods, combo, acc, and grade
    let scores_indices = filter_scores(top_type, scores, mode, &args);
    let amount = scores_indices.len();

    // Get all relevant maps from the database
    let map_ids: Vec<u32> = scores_indices
        .iter()
        .filter_map(|(_, s)| s.beatmap_id)
        .collect();
    let mut maps = match ctx.psql().get_beatmaps(&map_ids).await {
        Ok(maps) => maps,
        Err(why) => {
            unwind_error!(warn, why, "Error while getting maps from DB: {}");
            HashMap::default()
        }
    };

    debug!(
        "Found {}/{} beatmaps in DB",
        maps.len(),
        scores_indices.len()
    );
    let retrieving_msg = if scores_indices.len() - maps.len() > 10 {
        let content = format!(
            "Retrieving {} maps from the api...",
            scores_indices.len() - maps.len()
        );
        ctx.http
            .create_message(msg.channel_id)
            .content(content)?
            .await
            .ok()
    } else {
        None
    };

    // Retrieving all missing beatmaps
    let mut scores_data = Vec::with_capacity(scores_indices.len());
    let mut missing_maps = Vec::new();
    for (i, score) in scores_indices.into_iter() {
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
        scores_data.push((i, score, map));
    }

    // Accumulate all necessary data
    let content = match top_type {
        TopType::Top => {
            let cond = args.mods.is_some()
                || args.acc_min.is_some()
                || args.combo_min.is_some()
                || args.grade.is_some();
            if cond {
                let content = format!(
                    "Found {num} top score{plural} with the specified properties:",
                    num = amount,
                    plural = if amount != 1 { "s" } else { "" }
                );
                Some(content)
            } else {
                None
            }
        }
        TopType::Recent => Some(format!("Most recent scores in `{}`'s top100:", name)),
    };
    let pages = numbers::div_euclid(5, scores_data.len());
    let data = TopEmbed::new(&ctx, &user, scores_data.iter().take(5), mode, (1, pages)).await;

    if let Some(msg) = retrieving_msg {
        let _ = ctx.http.delete_message(msg.channel_id, msg.id).await;
    }

    // Creating the embed
    let embed = data.build().build()?;
    let create_msg = ctx.http.create_message(msg.channel_id).embed(embed)?;
    let response = match content {
        Some(content) => create_msg.content(content)?.await?,
        None => create_msg.await?,
    };

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
    let pagination = TopPagination::new(Arc::clone(&ctx), response, user, scores_data, mode);
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (top): {}")
        }
    });
    Ok(())
}

#[command]
#[short_desc("Display a user's top plays")]
#[long_desc(
    "Display a user's top plays.\n\
     Mods can be specified.\n\
     A combo and accuracy range can be specified with `-c` and `-a`. \
     After this keyword, you must specify either a number for min combo/acc, \
     or two numbers of the form `x..y` for min and max combo/acc.\n\
     The grade can be specified with `-grade`, followed by `SS`, `S`, `A`, `B`, `C`, or `D`.\n\
     Also, with `--a` I will sort by accuracy and with `--c` I will sort by combo."
)]
#[usage(
    "[username] [-a number[..number]] [-c number[..number]] [-grade SS/S/A/B/C/D] [mods] [--a/--c]"
)]
#[example(
    "badewanne3 -a 97.34..99.5 -grade A +hdhr --c",
    "vaxei -c 1234 -dt! --a"
)]
#[aliases("topscores", "osutop")]
pub async fn top(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::STD, TopType::Top, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's top mania plays")]
#[long_desc(
    "Display a user's top mania plays.\n\
     Mods can be specified.\n\
     A combo and accuracy range can be specified with `-c` and `-a`. \
     After this keyword, you must specify either a number for min combo/acc, \
     or two numbers of the form `x..y` for min and max combo/acc.\n\
     The grade can be specified with `-grade`, followed by `SS`, `S`, `A`, `B`, `C`, or `D`.\n\
     Also, with `--a` I will sort by accuracy and with `--c` I will sort by combo."
)]
#[usage(
    "[username] [-a number[..number]] [-c number[..number]] [-grade SS/S/A/B/C/D] [mods] [--a/--c]"
)]
#[example(
    "badewanne3 -a 97.34..99.5 -grade A +hdhr --c",
    "vaxei -c 1234 -dt! --a"
)]
#[aliases("topm")]
pub async fn topmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::MNA, TopType::Top, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's top taiko plays")]
#[long_desc(
    "Display a user's top taiko plays.\n\
     Mods can be specified.\n\
     A combo and accuracy range can be specified with `-c` and `-a`. \
     After this keyword, you must specify either a number for min combo/acc, \
     or two numbers of the form `x..y` for min and max combo/acc.\n\
     The grade can be specified with `-grade`, followed by `SS`, `S`, `A`, `B`, `C`, or `D`.\n\
     Also, with `--a` I will sort by accuracy and with `--c` I will sort by combo."
)]
#[usage(
    "[username] [-a number[..number]] [-c number[..number]] [-grade SS/S/A/B/C/D] [mods] [--a/--c]"
)]
#[example(
    "badewanne3 -a 97.34..99.5 -grade A +hdhr --c",
    "vaxei -c 1234 -dt! --a"
)]
#[aliases("topt")]
pub async fn toptaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::TKO, TopType::Top, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's top ctb plays")]
#[long_desc(
    "Display a user's top ctb plays.\n\
     Mods can be specified.\n\
     A combo and accuracy range can be specified with `-c` and `-a`. \
     After this keyword, you must specify either a number for min combo/acc, \
     or two numbers of the form `x..y` for min and max combo/acc.\n\
     The grade can be specified with `-grade`, followed by `SS`, `S`, `A`, `B`, `C`, or `D`.\n\
     Also, with `--a` I will sort by accuracy and with `--c` I will sort by combo."
)]
#[usage(
    "[username] [-a number[..number]] [-c number[..number]] [-grade SS/S/A/B/C/D] [mods] [--a/--c]"
)]
#[example(
    "badewanne3 -a 97.34..99.5 -grade A +hdhr --c",
    "vaxei -c 1234 -dt! --a"
)]
#[aliases("topc")]
pub async fn topctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::CTB, TopType::Top, ctx, msg, args).await
}

#[command]
#[short_desc("Sort a user's top plays by date")]
#[long_desc(
    "Display a user's most recent top plays.\n\
    Mods can be specified.\n\
    A combo and accuracy range can be specified with `-c` and `-a`. \
    After this keyword, you must specify either a number for min combo/acc, \
    or two numbers of the form `x..y` for min and max combo/acc.\n\
    The grade can be specified with `-grade`, followed by `SS`, `S`, `A`, `B`, `C`, or `D`."
)]
#[usage("[username] [-a number[..number]] [-c number[..number]] [-grade SS/S/A/B/C/D] [mods]")]
#[example("badewanne3 -a 97.34 -grade A +hdhr", "vaxei -c 1234 -dt!")]
#[aliases("rb")]
pub async fn recentbest(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::STD, TopType::Recent, ctx, msg, args).await
}

#[command]
#[short_desc("Sort a user's top mania plays by date")]
#[long_desc(
    "Display a user's most recent top mania plays.\n\
    Mods can be specified.\n\
    A combo and accuracy range can be specified with `-c` and `-a`. \
    After this keyword, you must specify either a number for min combo/acc, \
    or two numbers of the form `x..y` for min and max combo/acc.\n\
    The grade can be specified with `-grade`, followed by `SS`, `S`, `A`, `B`, `C`, or `D`."
)]
#[usage("[username] [-a number[..number]] [-c number[..number]] [-grade SS/S/A/B/C/D] [mods]")]
#[example("badewanne3 -a 97.34 -grade A +hdhr", "vaxei -c 1234 -dt!")]
#[aliases("rbm")]
pub async fn recentbestmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::MNA, TopType::Recent, ctx, msg, args).await
}

#[command]
#[short_desc("Sort a user's top taiko plays by date")]
#[long_desc(
    "Display a user's most recent top taiko plays.\n\
    Mods can be specified.\n\
    A combo and accuracy range can be specified with `-c` and `-a`. \
    After this keyword, you must specify either a number for min combo/acc, \
    or two numbers of the form `x..y` for min and max combo/acc.\n\
    The grade can be specified with `-grade`, followed by `SS`, `S`, `A`, `B`, `C`, or `D`."
)]
#[usage("[username] [-a number[..number]] [-c number[..number]] [-grade SS/S/A/B/C/D] [mods]")]
#[example("badewanne3 -a 97.34 -grade A +hdhr", "vaxei -c 1234 -dt!")]
#[aliases("rbt")]
pub async fn recentbesttaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::TKO, TopType::Recent, ctx, msg, args).await
}

#[command]
#[short_desc("Sort a user's top ctb plays by date")]
#[long_desc(
    "Display a user's most recent top ctb plays.\n\
    Mods can be specified.\n\
    A combo and accuracy range can be specified with `-c` and `-a`. \
    After this keyword, you must specify either a number for min combo/acc, \
    or two numbers of the form `x..y` for min and max combo/acc.\n\
    The grade can be specified with `-grade`, followed by `SS`, `S`, `A`, `B`, `C`, or `D`."
)]
#[usage("[username] [-a number[..number]] [-c number[..number]] [-grade SS/S/A/B/C/D] [mods]")]
#[example("badewanne3 -a 97.34 -grade A +hdhr", "vaxei -c 1234 -dt!")]
#[aliases("rbc")]
pub async fn recentbestctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::CTB, TopType::Recent, ctx, msg, args).await
}

#[derive(Eq, PartialEq, Copy, Clone)]
enum TopType {
    Top,
    Recent,
}

pub enum TopSortBy {
    None,
    Acc,
    Combo,
}

fn filter_scores(
    top_type: TopType,
    scores: Vec<Score>,
    mode: GameMode,
    args: &TopArgs,
) -> Vec<(usize, Score)> {
    let selection = args.mods;
    let grade = args.grade;
    let mut scores_indices: Vec<(usize, Score)> = scores
        .into_iter()
        .enumerate()
        .filter(|(_, s)| {
            if let Some(grade) = grade {
                if !s.grade.eq_letter(grade) {
                    return false;
                }
            }

            let mod_bool = match selection {
                None => true,
                Some(ModSelection::Exact(mods)) => {
                    if mods.is_empty() {
                        s.enabled_mods.is_empty()
                    } else {
                        mods == s.enabled_mods
                    }
                }
                Some(ModSelection::Include(mods)) => {
                    if mods.is_empty() {
                        s.enabled_mods.is_empty()
                    } else {
                        s.enabled_mods.contains(mods)
                    }
                }
                Some(ModSelection::Exclude(mods)) => {
                    if mods.is_empty() && s.enabled_mods.is_empty() {
                        false
                    } else if mods.is_empty() {
                        true
                    } else {
                        !s.enabled_mods.contains(mods)
                    }
                }
            };
            if !mod_bool {
                return false;
            }

            let acc = s.accuracy(mode);
            let acc_bool = match (args.acc_min, args.acc_max) {
                (Some(a), _) if a > acc => false,
                (_, Some(a)) if a < acc => false,
                _ => true,
            };

            let combo_bool = match (args.combo_min, args.combo_max) {
                (Some(c), _) if c > s.max_combo => false,
                (_, Some(c)) if c < s.max_combo => false,
                _ => true,
            };

            acc_bool && combo_bool
        })
        .collect();
    match args.sort_by {
        TopSortBy::Acc => {
            let acc_cache: HashMap<_, _> = scores_indices
                .iter()
                .map(|(i, s)| (*i, s.accuracy(mode)))
                .collect();
            scores_indices.sort_unstable_by(|(a, _), (b, _)| {
                acc_cache
                    .get(&b)
                    .partial_cmp(&acc_cache.get(&a))
                    .unwrap_or(Ordering::Equal)
            });
        }
        TopSortBy::Combo => {
            scores_indices.sort_unstable_by(|(_, a), (_, b)| b.max_combo.cmp(&a.max_combo))
        }
        TopSortBy::None => {}
    }
    if top_type == TopType::Recent {
        scores_indices.sort_unstable_by(|(_, a), (_, b)| b.date.cmp(&a.date));
    }
    scores_indices.iter_mut().for_each(|(i, _)| *i += 1);
    scores_indices
}
