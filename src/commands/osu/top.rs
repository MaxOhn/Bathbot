use crate::{
    arguments::{ModSelection, TopArgs},
    database::MySQL,
    embeds::BasicEmbedData,
    util::{discord, globals::OSU_API_ISSUE},
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::UserRequest,
    models::{Beatmap, GameMode, GameMods, Score, User},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::collections::HashMap;
use tokio::runtime::Runtime;

fn top_send(
    mode: GameMode,
    top_type: TopType,
    ctx: &mut Context,
    msg: &Message,
    args: Args,
) -> CommandResult {
    let args = match TopArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => {
            let response = msg.channel_id.say(&ctx.http, err_msg)?;
            discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
            return Ok(());
        }
    };
    let (mods, selection) = args
        .mods
        .unwrap_or_else(|| (GameMods::default(), ModSelection::None));
    let combo = args.combo.unwrap_or(0);
    let acc = args.acc.unwrap_or(0.0);
    let grade = args.grade;
    let name = if let Some(name) = args.name {
        name
    } else {
        let data = ctx.data.read();
        let links = data
            .get::<DiscordLinks>()
            .expect("Could not get DiscordLinks");
        match links.get(msg.author.id.as_u64()) {
            Some(name) => name.clone(),
            None => {
                msg.channel_id.say(
                    &ctx.http,
                    "Either specify an osu name or link your discord \
                     to an osu profile via `<link osuname`",
                )?;
                return Ok(());
            }
        }
    };
    let reverse = args.reverse;
    let mut rt = Runtime::new().unwrap();

    // Retrieve the user and its top scores
    let (user, scores): (User, Vec<Score>) = {
        let user_req = UserRequest::with_username(&name).mode(mode);
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let user = match rt.block_on(user_req.queue_single(&osu)) {
            Ok(result) => match result {
                Some(user) => user,
                None => {
                    msg.channel_id
                        .say(&ctx.http, format!("User `{}` was not found", name))?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        let scores = match rt.block_on(user.get_top_scores(&osu, 100, mode)) {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        (user, scores)
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
                ModSelection::Exact => mods == s.enabled_mods,
                ModSelection::Includes => mods.iter().all(|m| s.enabled_mods.contains(&m)),
                ModSelection::Excludes => mods.iter().all(|m| !s.enabled_mods.contains(&m)),
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
    let mut amount = scores_indices.len();
    if top_type != TopType::Sotarks {
        if top_type == TopType::Recent {
            scores_indices.sort_by(|(_, a), (_, b)| b.date.cmp(&a.date));
            amount = scores_indices.len().min(5);
        }
        if reverse {
            scores_indices = scores_indices.into_iter().rev().collect();
        }
        scores_indices = scores_indices[..amount.min(5)].to_vec();
    }
    scores_indices.iter_mut().for_each(|(i, _)| *i += 1);

    // Get all relevant maps from the database
    let map_ids: Vec<u32> = scores_indices
        .iter()
        .map(|(_, s)| s.beatmap_id.unwrap())
        .collect();
    let mut maps = {
        let data = ctx.data.read();
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

    // Retrieving all missing beatmaps
    let res = rt.block_on(async {
        let dont_filter_sotarks = top_type != TopType::Sotarks;
        let mut tuples = Vec::with_capacity(scores_indices.len());
        let mut missing_indices = Vec::with_capacity(scores_indices.len());
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        for (i, score) in scores_indices.into_iter() {
            let map_id = score.beatmap_id.unwrap();
            let map = if maps.contains_key(&map_id) {
                maps.remove(&map_id).unwrap()
            } else {
                missing_indices.push(i);
                score.get_beatmap(osu).await.or_else(|e| {
                    Err(CommandError(format!(
                        "Error while retrieving Beatmap of score: {}",
                        e
                    )))
                })?
            };
            if dont_filter_sotarks || &map.creator == "Sotarks" {
                tuples.push((i, score, map));
            }
        }
        Ok((tuples, missing_indices))
    });
    let (mut scores_data, missing_maps): (Vec<_>, Option<Vec<Beatmap>>) = match res {
        Ok((scores_data, missing_indices)) => {
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
            (scores_data, missing_maps)
        }
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
        }
    };

    // Accumulate all necessary data
    let content = match top_type {
        TopType::Top => {
            let mut content = format!(
                "Found {num} top score{plural} with the specified properties",
                num = amount,
                plural = if amount != 1 { "s" } else { "" }
            );
            if amount > 5 {
                content.push_str(&format!(
                    ", here's the {} 5 of them:",
                    if reverse { "last" } else { "top" }
                ));
            } else {
                content.push(':');
            }
            content
        }
        TopType::Recent => format!(
            "Here are the {num} {adj} recent scores in `{name}`'s top 100",
            num = scores_data.len(),
            adj = if reverse { "least" } else { "most" },
            name = name,
        ),
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
                _ => content.push_str(", so sad \\:'("),
            }
            if amount > 5 {
                content.push_str(&format!(
                    "\nHere are the {} 5:",
                    if reverse { "last" } else { "top" }
                ));
            }
            scores_data = scores_data[..amount.min(5)].to_vec();
            content
        }
    };
    let data = match BasicEmbedData::create_top(user, scores_data, mode, &ctx) {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id.say(
                &ctx.http,
                "Some issue while calculating top data, blame bade",
            )?;
            return Err(CommandError::from(why.to_string()));
        }
    };

    // Creating the embed
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.content(content).embed(|e| data.build(e)));

    // Add missing maps to database
    if let Some(maps) = missing_maps {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.insert_beatmaps(maps) {
            warn!(
                "Could not add missing maps of top command to database: {}",
                why
            );
        }
    }

    // Save the response owner
    discord::save_response_owner(response?.id, msg.author.id, ctx.data.clone());
    Ok(())
}

#[command]
#[description = "Display a user's top plays. \
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`. \
                 With `-reverse` I will reverse the resulting list."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [-reverse]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr"]
#[example = "vaxei -c 1234 -dt!"]
#[aliases("topscores", "osutop")]
pub fn top(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::STD, TopType::Top, ctx, msg, args)
}

#[command]
#[description = "Display a user's top mania plays. \
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`. \
                 With `-reverse` I will reverse the resulting list."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [-reverse]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr"]
#[example = "vaxei -c 1234 -dt!"]
#[aliases("topm")]
pub fn topmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::MNA, TopType::Top, ctx, msg, args)
}

#[command]
#[description = "Display a user's top taiko plays. \
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`. \
                 With `-reverse` I will reverse the resulting list."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [-reverse]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr"]
#[example = "vaxei -c 1234 -dt!"]
#[aliases("topt")]
pub fn toptaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::TKO, TopType::Top, ctx, msg, args)
}

#[command]
#[description = "Display a user's top ctb plays. \
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`. \
                 With `-reverse` I will reverse the resulting list."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [-reverse]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr"]
#[example = "vaxei -c 1234 -dt!"]
#[aliases("topc")]
pub fn topctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::CTB, TopType::Top, ctx, msg, args)
}

#[command]
#[description = "Display a user's most recent top plays. \
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`. \
                 With `-reverse` I will reverse the resulting list."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [-reverse]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr"]
#[example = "vaxei -c 1234 -dt!"]
#[aliases("rb")]
pub fn recentbest(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::STD, TopType::Recent, ctx, msg, args)
}

#[command]
#[description = "Display a user's most recent top mania plays. \
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`. \
                 With `-reverse` I will reverse the resulting list."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [-reverse]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr"]
#[example = "vaxei -c 1234 -dt!"]
#[aliases("rbm")]
pub fn recentbestmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::MNA, TopType::Recent, ctx, msg, args)
}

#[command]
#[description = "Display a user's most recent top taiko plays. \
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`. \
                 With `-reverse` I will reverse the resulting list."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [-reverse]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr"]
#[example = "vaxei -c 1234 -dt!"]
#[aliases("rbt")]
pub fn recentbesttaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::TKO, TopType::Recent, ctx, msg, args)
}

#[command]
#[description = "Display a user's most recent top ctb plays. \
                 Mods can be specified, aswell as minimal acc \
                 with `-a`, combo with `-c`, and a grade with `-grade`. \
                 With `-reverse` I will reverse the resulting list."]
#[usage = "[username] [-a number] [-c number] [-grade SS/S/A/B/C/D] [+mods] [-reverse]"]
#[example = "badewanne3 -a 97.34 -grade A +hdhr"]
#[example = "vaxei -c 1234 -dt!"]
#[aliases("rbc")]
pub fn recentbestctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::CTB, TopType::Recent, ctx, msg, args)
}

#[command]
#[description = "Display how many top play maps of a user are made by Sotarks"]
#[usage = "[username]"]
#[example = "badewanne3"]
pub fn sotarks(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::STD, TopType::Sotarks, ctx, msg, args)
}

#[derive(Eq, PartialEq)]
enum TopType {
    Top,
    Recent,
    Sotarks,
}
