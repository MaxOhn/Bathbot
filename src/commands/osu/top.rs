use crate::{
    arguments::{ModSelection, TopArgs},
    database::MySQL,
    embeds::BasicEmbedData,
    util::{discord, globals::OSU_API_ISSUE, numbers},
    DiscordLinks, Osu,
};

use rayon::prelude::*;
use rosu::{
    backend::requests::UserRequest,
    models::{Beatmap, GameMod, GameMode, GameMods, Score, User},
};
use serenity::{
    cache::CacheRwLock,
    collector::{ReactionAction, ReactionCollectorBuilder},
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::channel::{Message, ReactionType},
    prelude::{Context, RwLock, ShareMap},
};
use std::{cmp::Ordering, collections::HashMap, sync::Arc, time::Duration};

#[allow(clippy::cognitive_complexity)]
async fn top_send(
    mode: GameMode,
    top_type: TopType,
    ctx: &mut Context,
    msg: &Message,
    args: Args,
) -> CommandResult {
    let args = match TopArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => {
            let response = msg.channel_id.say(&ctx.http, err_msg).await?;
            discord::reaction_deletion(&ctx, response, msg.author.id);
            return Ok(());
        }
    };
    let (mut mods, selection) = args
        .mods
        .unwrap_or_else(|| (GameMods::default(), ModSelection::None));
    let combo = args.combo.unwrap_or(0);
    let acc = args.acc.unwrap_or(0.0);
    let grade = args.grade;
    let name = if let Some(name) = args.name {
        name
    } else {
        let data = ctx.data.read().await;
        let links = data
            .get::<DiscordLinks>()
            .expect("Could not get DiscordLinks");
        match links.get(msg.author.id.as_u64()) {
            Some(name) => name.clone(),
            None => {
                msg.channel_id
                    .say(
                        &ctx.http,
                        "Either specify an osu name or link your discord \
                     to an osu profile via `<link osuname`",
                    )
                    .await?;
                return Ok(());
            }
        }
    };

    // Retrieve the user and its top scores
    let (user, scores): (User, Vec<Score>) = {
        let user_req = UserRequest::with_username(&name).mode(mode);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let user = match user_req.queue_single(&osu).await {
            Ok(result) => match result {
                Some(user) => user,
                None => {
                    msg.channel_id
                        .say(&ctx.http, format!("User `{}` was not found", name))
                        .await?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        let scores = match user.get_top_scores(&osu, 100, mode).await {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        (user, scores)
    };
    let contains_nm = mods.remove(&GameMod::NoMod).is_some();

    // Filter scores according to mods, combo, acc, and grade
    let mut scores_indices: Vec<(usize, Score)> = scores
        .into_par_iter()
        .enumerate()
        .filter(|(_, s)| {
            if let Some(grade) = grade {
                if !s.grade.eq_letter(grade) {
                    return false;
                }
            }
            let mod_bool = match selection {
                ModSelection::None => true,
                ModSelection::Exact => {
                    if contains_nm {
                        s.enabled_mods.is_empty()
                    } else {
                        mods == s.enabled_mods
                    }
                }
                ModSelection::Includes => {
                    if contains_nm {
                        s.enabled_mods.is_empty()
                    } else {
                        mods.iter().all(|m| s.enabled_mods.contains(m))
                    }
                }
                ModSelection::Excludes => {
                    if contains_nm && s.enabled_mods.is_empty() {
                        false
                    } else {
                        mods.iter().all(|m| !s.enabled_mods.contains(m))
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
        .map(|(_, s)| s.beatmap_id.unwrap())
        .collect();
    let mut maps = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql
            .get_beatmaps(&map_ids)
            .unwrap_or_else(|_| HashMap::default())
    };
    info!(
        "Found {}/{} beatmaps in the database",
        maps.len(),
        scores_indices.len()
    );
    let retrieving_msg = if scores_indices.len() - maps.len() > 15 {
        Some(
            msg.channel_id
                .say(
                    &ctx.http,
                    format!(
                        "Retrieving {} maps from the api...",
                        scores_indices.len() - maps.len()
                    ),
                )
                .await?,
        )
    } else {
        None
    };

    // Retrieving all missing beatmaps
    let mut scores_data = Vec::with_capacity(scores_indices.len());
    let mut missing_indices = Vec::with_capacity(scores_indices.len());
    {
        let dont_filter_sotarks = top_type != TopType::Sotarks;
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        for (i, score) in scores_indices.into_iter() {
            let map_id = score.beatmap_id.unwrap();
            let map = if maps.contains_key(&map_id) {
                maps.remove(&map_id).unwrap()
            } else {
                missing_indices.push(i);
                match score.get_beatmap(osu).await {
                    Ok(map) => map,
                    Err(why) => {
                        msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                        return Err(CommandError::from(why.to_string()));
                    }
                }
            };
            if dont_filter_sotarks || &map.creator == "Sotarks" {
                scores_data.push((i, score, map));
            }
        }
    }
    let missing_maps = if missing_indices.is_empty() || scores_data.is_empty() {
        None
    } else {
        Some(
            scores_data
                .iter()
                .filter(|(i, ..)| missing_indices.contains(i))
                .map(|(.., map)| map.clone())
                .collect(),
        )
    };

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
                "I found {amount} Sotarks map{plural} in `{name}`'s top 100",
                amount = amount,
                plural = if amount != 1 { "s" } else { "" },
                name = name
            );
            match amount {
                0 => content.push_str(", proud of you \\:)"),
                n if n <= 5 => content.push_str(", kinda sad \\:/"),
                n if n <= 10 => content.push_str(", pretty sad \\:("),
                n if n <= 15 => content.push_str(", really sad \\:(("),
                _ => content.push_str(", just sad \\:'("),
            }
            content
        }
    };
    let pages = numbers::div_euclid(5, amount);
    let data =
        match BasicEmbedData::create_top(&user, scores_data.iter().take(5), mode, (1, pages), &ctx)
            .await
        {
            Ok(data) => data,
            Err(why) => {
                msg.channel_id
                    .say(
                        &ctx.http,
                        "Some issue while calculating top data, blame bade",
                    )
                    .await?;
                return Err(CommandError::from(why.to_string()));
            }
        };

    if let Some(msg) = retrieving_msg {
        msg.delete(&ctx.http).await?;
    }

    // Creating the embed
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.content(content)
                // .reactions(reactions.iter().copied())
                .embed(|e| data.build(e))
        })
        .await;

    // Add missing maps to database
    if let Some(maps) = missing_maps {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.insert_beatmaps(maps) {
            warn!(
                "Could not add missing maps of top command to database: {}",
                why
            );
        }
    }
    let mut response = response?;

    // Collect reactions of author on the response
    let mut collector = ReactionCollectorBuilder::new(&ctx)
        .author_id(msg.author.id)
        .message_id(response.id)
        .timeout(Duration::from_secs(90))
        .await;
    let mut idx = 0;

    // Add initial reactions
    let reactions = ["⏮️", "⏪", "⏩", "⏭️"];
    for &reaction in reactions.iter() {
        response.react(&ctx.http, reaction).await?;
    }

    // Check if the author wants to edit the response
    let http = Arc::clone(&ctx.http);
    let cache = ctx.cache.clone();
    let data = Arc::clone(&ctx.data);
    tokio::spawn(async move {
        while let Some(reaction) = collector.receive_one().await {
            if let ReactionAction::Added(reaction) = &*reaction {
                if let ReactionType::Unicode(reaction_name) = &reaction.emoji {
                    let reaction_data = reaction_data(
                        reaction_name.as_str(),
                        &mut idx,
                        &user,
                        &scores_data,
                        mode,
                        &cache,
                        &data,
                    );
                    match reaction_data.await {
                        ReactionData::None => {}
                        ReactionData::Delete => response.delete((&cache, &*http)).await?,
                        ReactionData::Data(data) => {
                            response
                                .edit((&cache, &*http), |m| m.embed(|e| data.build(e)))
                                .await?
                        }
                    }
                }
            }
        }
        for &reaction in reactions.iter() {
            response
                .channel_id
                .delete_reaction(&http, response.id, None, reaction)
                .await?;
        }
        Ok::<_, serenity::Error>(())
    });
    Ok(())
}

enum ReactionData {
    Data(Box<BasicEmbedData>),
    Delete,
    None,
}

async fn reaction_data(
    reaction: &str,
    idx: &mut usize,
    user: &User,
    scores: &[(usize, Score, Beatmap)],
    mode: GameMode,
    cache: &CacheRwLock,
    data: &Arc<RwLock<ShareMap>>,
) -> ReactionData {
    let amount = scores.len();
    let pages = numbers::div_euclid(5, amount);
    match reaction {
        "❌" => ReactionData::Delete,
        "⏮️" => {
            if *idx > 0 {
                *idx = 0;
                BasicEmbedData::create_top(
                    &user,
                    scores.iter().take(5),
                    mode,
                    (*idx / 5 + 1, pages),
                    (cache, data),
                )
                .await
                .map(|data| ReactionData::Data(Box::new(data)))
                .unwrap_or_else(|why| {
                    warn!("Error editing top data at idx {}/{}: {}", idx, amount, why);
                    ReactionData::None
                })
            } else {
                ReactionData::None
            }
        }
        "⏪" => {
            if *idx > 0 {
                *idx = idx.saturating_sub(5);
                BasicEmbedData::create_top(
                    &user,
                    scores.iter().skip(*idx).take(5),
                    mode,
                    (*idx / 5 + 1, pages),
                    (cache, data),
                )
                .await
                .map(|data| ReactionData::Data(Box::new(data)))
                .unwrap_or_else(|why| {
                    warn!("Error editing top data at idx {}/{}: {}", idx, amount, why);
                    ReactionData::None
                })
            } else {
                ReactionData::None
            }
        }
        "⏩" => {
            let limit = if amount % 5 == 0 {
                amount - 5
            } else {
                amount - amount % 5
            };
            if *idx < limit {
                *idx = limit.min(*idx + 5);
                BasicEmbedData::create_top(
                    &user,
                    scores.iter().skip(*idx).take(5),
                    mode,
                    (*idx / 5 + 1, pages),
                    (cache, data),
                )
                .await
                .map(|data| ReactionData::Data(Box::new(data)))
                .unwrap_or_else(|why| {
                    warn!("Error editing top data at idx {}/{}: {}", idx, amount, why);
                    ReactionData::None
                })
            } else {
                ReactionData::None
            }
        }
        "⏭️" => {
            let limit = if amount % 5 == 0 {
                amount - 5
            } else {
                amount - amount % 5
            };
            if *idx < limit {
                *idx = limit;
                BasicEmbedData::create_top(
                    &user,
                    scores.iter().skip(*idx).take(5),
                    mode,
                    (*idx / 5 + 1, pages),
                    (cache, data),
                )
                .await
                .map(|data| ReactionData::Data(Box::new(data)))
                .unwrap_or_else(|why| {
                    warn!("Error editing top data at idx {}/{}: {}", idx, amount, why);
                    ReactionData::None
                })
            } else {
                ReactionData::None
            }
        }
        _ => ReactionData::None,
    }
}

#[command]
#[description = "Display a user's top plays.\n\
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`.\n\
                 Also, with `--a` I will sort by accuracy and with `--c` I will sort by combo."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [--a/--c]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr --c"]
#[example = "vaxei -c 1234 -dt! --a"]
#[aliases("topscores", "osutop")]
pub async fn top(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::STD, TopType::Top, ctx, msg, args).await
}

#[command]
#[description = "Display a user's top mania plays.\n\
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`.\n\
                 Also, with `--a` I will sort by accuracy and with `--c` I will sort by combo."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [--a/--c]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr --c"]
#[example = "vaxei -c 1234 -dt! --a"]
#[aliases("topm")]
pub async fn topmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::MNA, TopType::Top, ctx, msg, args).await
}

#[command]
#[description = "Display a user's top taiko plays.\n\
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`.\n\
                 Also, with `--a` I will sort by accuracy and with `--c` I will sort by combo."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [--a/--c]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr --c"]
#[example = "vaxei -c 1234 -dt! --a"]
#[aliases("topt")]
pub async fn toptaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::TKO, TopType::Top, ctx, msg, args).await
}

#[command]
#[description = "Display a user's top ctb plays.\n\
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`.\n\
                 Also, with `--a` I will sort by accuracy and with `--c` I will sort by combo."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [--a/--c]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr --c"]
#[example = "vaxei -c 1234 -dt! --a"]
#[aliases("topc")]
pub async fn topctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::CTB, TopType::Top, ctx, msg, args).await
}

#[command]
#[description = "Display a user's most recent top plays.\n\
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr"]
#[example = "vaxei -c 1234 -dt!"]
#[aliases("rb")]
pub async fn recentbest(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::STD, TopType::Recent, ctx, msg, args).await
}

#[command]
#[description = "Display a user's most recent top mania plays.\n\
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr"]
#[example = "vaxei -c 1234 -dt!"]
#[aliases("rbm")]
pub async fn recentbestmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::MNA, TopType::Recent, ctx, msg, args).await
}

#[command]
#[description = "Display a user's most recent top taiko plays.\n\
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr"]
#[example = "vaxei -c 1234 -dt!"]
#[aliases("rbt")]
pub async fn recentbesttaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::TKO, TopType::Recent, ctx, msg, args).await
}

#[command]
#[description = "Display a user's most recent top ctb plays.\n\
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr"]
#[example = "vaxei -c 1234 -dt!"]
#[aliases("rbc")]
pub async fn recentbestctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::CTB, TopType::Recent, ctx, msg, args).await
}

#[command]
#[description = "Display how many top play maps of a user are made by Sotarks"]
#[usage = "[username]"]
#[example = "badewanne3"]
pub async fn sotarks(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::STD, TopType::Sotarks, ctx, msg, args).await
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
