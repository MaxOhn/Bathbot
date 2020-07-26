use super::require_link;
use crate::{
    arguments::{Args, TopArgs},
    embeds::{EmbedData, TopEmbed},
    pagination::{Pagination, TopPagination},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        numbers,
        osu::ModSelection,
        MessageExt,
    },
    BotResult, Context,
};

use regex::Regex;
use rosu::{
    backend::{BestRequest, UserRequest},
    models::{Beatmap, GameMode, GameMods, Score, User},
};
use std::{cmp::Ordering, collections::HashMap, sync::Arc};
use twilight::model::channel::Message;

#[allow(clippy::cognitive_complexity)]
async fn top_main(
    mode: GameMode,
    top_type: TopType,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = match TopArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => {
            msg.respond(&ctx, err_msg).await?;
            return Ok(());
        }
    };
    let selection = args.mods.unwrap_or_else(|| ModSelection::None);
    let combo = args.combo.unwrap_or(0);
    let acc = args.acc.unwrap_or(0.0);
    let grade = args.grade;
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return require_link(&ctx, msg).await,
    };

    // Retrieve the user and their top scores
    let scores_fut = BestRequest::with_username(&name)
        .mode(mode)
        .limit(100)
        .queue(&ctx.clients.osu);
    let join_result = tokio::try_join!(ctx.osu_user(&name, mode), scores_fut);
    let (user, scores) = match join_result {
        Ok((Some(user), scores)) => (user, scores),
        Ok((None, _)) => {
            let content = format!("User `{}` was not found", name);
            return msg.respond(&ctx, content).await;
        }
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why.into());
        }
    };

    // Filter scores according to mods, combo, acc, and grade
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
                ModSelection::None => true,
                ModSelection::Exact(mods) => {
                    if mods.is_empty() {
                        s.enabled_mods.is_empty()
                    } else {
                        mods == s.enabled_mods
                    }
                }
                ModSelection::Include(mods) => {
                    if mods.is_empty() {
                        s.enabled_mods.is_empty()
                    } else {
                        s.enabled_mods.contains(mods)
                    }
                }
                ModSelection::Exclude(mods) => {
                    if mods.is_empty() && s.enabled_mods.is_empty() {
                        false
                    } else {
                        !s.enabled_mods.contains(mods)
                    }
                }
            };
            if !mod_bool {
                return false;
            }
            let acc_bool = if acc > 0.0 {
                s.accuracy(mode) >= acc
            } else {
                true
            };
            acc_bool && s.max_combo >= combo
        })
        .collect();
    let amount = scores_indices.len();
    match args.sort_by {
        TopSortBy::Acc => {
            let acc_cache: HashMap<_, _> = scores_indices
                .iter()
                .map(|(i, s)| (*i, s.accuracy(mode)))
                .collect();
            scores_indices.sort_by(|(a, _), (b, _)| {
                acc_cache
                    .get(&b)
                    .unwrap()
                    .partial_cmp(acc_cache.get(&a).unwrap())
                    .unwrap_or(Ordering::Equal)
            });
        }
        TopSortBy::Combo => scores_indices.sort_by(|(_, a), (_, b)| b.max_combo.cmp(&a.max_combo)),
        TopSortBy::None => {}
    }
    if top_type == TopType::Recent {
        scores_indices.sort_by(|(_, a), (_, b)| b.date.cmp(&a.date));
    }
    scores_indices.iter_mut().for_each(|(i, _)| *i += 1);

    // Get all relevant maps from the database
    let map_ids: Vec<u32> = scores_indices
        .iter()
        .filter_map(|(_, s)| s.beatmap_id)
        .collect();
    let mut maps = match ctx.clients.psql.get_beatmaps(&map_ids).await {
        Ok(maps) => maps,
        Err(why) => {
            warn!("Error while getting maps from DB: {}", why);
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
    // let mut missing_indices = Vec::with_capacity(scores_indices.len());
    let mut missing_maps = None;
    {
        let dont_filter_sotarks = top_type != TopType::Sotarks;
        let mut curr_missing_maps = Vec::with_capacity(8);
        for (i, score) in scores_indices.into_iter() {
            let map_id = score.beatmap_id.unwrap();
            let map = if maps.contains_key(&map_id) {
                maps.remove(&map_id).unwrap()
            } else {
                match score.get_beatmap(&ctx.clients.osu).await {
                    Ok(map) => {
                        curr_missing_maps.push(map.clone());
                        map
                    }
                    Err(why) => {
                        msg.respond(&ctx, OSU_API_ISSUE).await?;
                        return Err(why.into());
                    }
                }
            };
            if dont_filter_sotarks || is_sotarks_map(&map) {
                scores_data.push((i, score, map));
            }
        }
        if !curr_missing_maps.is_empty() {
            missing_maps = Some(curr_missing_maps);
        }
    }

    // Accumulate all necessary data
    let content = match top_type {
        TopType::Top => format!(
            "Found {num} top score{plural} with the specified properties:",
            num = amount,
            plural = if amount != 1 { "s" } else { "" }
        ),
        TopType::Recent => format!("Most recent scores in `{}`'s top 100:", name),
        TopType::Sotarks => {
            let amount = scores_data.len();
            let mut content = format!(
                "I found {amount} Sotarks map{plural} in `{name}`'s top 100, ",
                amount = amount,
                plural = if amount != 1 { "s" } else { "" },
                name = name
            );
            match amount {
                0 => content.push_str("proud of you \\:)"),
                n if n <= 5 => content.push_str("kinda sad \\:/"),
                n if n <= 10 => content.push_str("pretty sad \\:("),
                n if n <= 15 => content.push_str("really sad \\:(("),
                _ => content.push_str("just sad \\:'("),
            }
            content
        }
    };
    let pages = numbers::div_euclid(5, scores_data.len());
    let data = match TopEmbed::new(
        &user,
        scores_data.iter().take(5),
        mode,
        (1, pages),
        ctx.clone(),
    )
    .await
    {
        Ok(data) => data,
        Err(why) => {
            msg.respond(&ctx, GENERAL_ISSUE).await?;
            return Err(why);
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
        .content(content)?
        .embed(embed)?
        .await?;

    // Add missing maps to database
    if let Some(maps) = missing_maps {
        let len = maps.len();
        match ctx.clients.psql.insert_beatmaps(&maps).await {
            Ok(_) if len == 1 => {}
            Ok(_) => info!("Added {} maps to DB", len),
            Err(why) => warn!("Error while adding maps to DB: {}", why),
        }
    }

    // Skip pagination if too few entries
    if scores_data.len() <= 5 {
        response.reaction_delete(&ctx, msg.author.id);
        return Ok(());
    }

    // Pagination
    let pagination = TopPagination::new(ctx.clone(), response, user, scores_data, mode);
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 90).await {
            warn!("Pagination error: {}", why)
        }
    });
    Ok(())
}

#[command]
#[short_desc("Display a user's top plays")]
#[long_desc(
    "Display a user's top plays.\n\
     Mods can be specified, aswell as minimal acc \
     with `-a`, combo with `-c`, and a grade with `-grade`.\n\
     Also, with `--a` I will sort by accuracy and with `--c` I will sort by combo."
)]
#[usage("[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [--a/--c]")]
#[example("badewanne3 -a 97.34 -grade A +hdhr --c")]
#[example("vaxei -c 1234 -dt! --a")]
#[aliases("topscores", "osutop")]
pub async fn top(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::STD, TopType::Top, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's top plays")]
#[long_desc(
    "Display a user's top mania plays.\n\
     Mods can be specified, aswell as minimal acc \
     with `-a`, combo with `-c`, and a grade with `-grade`.\n\
     Also, with `--a` I will sort by accuracy and with `--c` I will sort by combo."
)]
#[usage("[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [--a/--c]")]
#[example("badewanne3 -a 97.34 -grade A +hdhr --c")]
#[example("vaxei -c 1234 -dt! --a")]
#[aliases("topm")]
pub async fn topmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::MNA, TopType::Top, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's top plays")]
#[long_desc(
    "Display a user's top taiko plays.\n\
     Mods can be specified, aswell as minimal acc \
     with `-a`, combo with `-c`, and a grade with `-grade`.\n\
     Also, with `--a` I will sort by accuracy and with `--c` I will sort by combo."
)]
#[usage("[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [--a/--c]")]
#[example("badewanne3 -a 97.34 -grade A +hdhr --c")]
#[example("vaxei -c 1234 -dt! --a")]
#[aliases("topt")]
pub async fn toptaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::TKO, TopType::Top, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's top plays")]
#[long_desc(
    "Display a user's top ctb plays.\n\
     Mods can be specified, aswell as minimal acc \
     with `-a`, combo with `-c`, and a grade with `-grade`.\n\
     Also, with `--a` I will sort by accuracy and with `--c` I will sort by combo."
)]
#[usage("[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [--a/--c]")]
#[example("badewanne3 -a 97.34 -grade A +hdhr --c")]
#[example("vaxei -c 1234 -dt! --a")]
#[aliases("topc")]
pub async fn topctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::CTB, TopType::Top, ctx, msg, args).await
}

#[command]
#[short_desc("Sort a user's top plays by date")]
#[long_desc(
    "Display a user's most recent top plays.\n\
     Mods can be specified, aswell as minimal acc \
     with `-a`, combo with `-c`, and a grade with `-grade`."
)]
#[usage("[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods]")]
#[example("badewanne3 -a 97.34 -grade A +hdhr")]
#[example("vaxei -c 1234 -dt!")]
#[aliases("rb")]
pub async fn recentbest(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::STD, TopType::Recent, ctx, msg, args).await
}

#[command]
#[short_desc("Sort a user's top plays by date")]
#[long_desc(
    "Display a user's most recent top mania plays.\n\
     Mods can be specified, aswell as minimal acc \
     with `-a`, combo with `-c`, and a grade with `-grade`."
)]
#[usage("[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods]")]
#[example("badewanne3 -a 97.34 -grade A +hdhr")]
#[example("vaxei -c 1234 -dt!")]
#[aliases("rbm")]
pub async fn recentbestmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::MNA, TopType::Recent, ctx, msg, args).await
}

#[command]
#[short_desc("Sort a user's top plays by date")]
#[long_desc(
    "Display a user's most recent top taiko plays.\n\
     Mods can be specified, aswell as minimal acc \
     with `-a`, combo with `-c`, and a grade with `-grade`."
)]
#[usage("[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods]")]
#[example("badewanne3 -a 97.34 -grade A +hdhr")]
#[example("vaxei -c 1234 -dt!")]
#[aliases("rbt")]
pub async fn recentbesttaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::TKO, TopType::Recent, ctx, msg, args).await
}

#[command]
#[short_desc("Sort a user's top plays by date")]
#[long_desc(
    "Display a user's most recent top ctb plays.\n\
     Mods can be specified, aswell as minimal acc \
     with `-a`, combo with `-c`, and a grade with `-grade`."
)]
#[usage("[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods]")]
#[example("badewanne3 -a 97.34 -grade A +hdhr")]
#[example("vaxei -c 1234 -dt!")]
#[aliases("rbc")]
pub async fn recentbestctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::CTB, TopType::Recent, ctx, msg, args).await
}

#[command]
#[short_desc("How many maps of a user's top100 are made by Sotarks?")]
#[usage("[username]")]
#[example("badewanne3")]
pub async fn sotarks(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    top_main(GameMode::STD, TopType::Sotarks, ctx, msg, args).await
}

#[derive(Eq, PartialEq)]
enum TopType {
    Top,
    Recent,
    Sotarks,
}

pub enum TopSortBy {
    None,
    Acc,
    Combo,
}

fn is_sotarks_map(map: &Beatmap) -> bool {
    let version = map.version.to_lowercase();
    let guest_diff = if map.creator.to_lowercase() == "sotarks" {
        !Regex::new(".*'s? (easy|normal|hard|insane|expert|extra|extreme)")
            .unwrap()
            .is_match(&version)
    } else {
        false
    };
    version.contains("sotarks") || guest_diff
}
