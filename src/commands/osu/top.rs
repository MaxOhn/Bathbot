use crate::{
    commands::{ArgParser, ModSelection},
    messages::{BotEmbed, MapMultiData},
    util::globals::OSU_API_ISSUE,
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::{OsuArgs, OsuRequest, UserArgs, UserBestArgs},
    models::{GameMode, GameMods, Grade, Score, User},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::{convert::TryFrom, str::FromStr};
use tokio::runtime::Runtime;

fn top_send(mode: GameMode, ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let mut arg_parser = ArgParser::new(args);
    let (mods, selection) = if let Some((m, s)) = arg_parser.get_mods() {
        let mods = match GameMods::try_from(m.as_ref()) {
            Ok(mods) => mods,
            Err(_) => {
                msg.channel_id
                    .say(&ctx.http, "Could not parse given mods")?;
                return Ok(());
            }
        };
        (mods, s)
    } else {
        (GameMods::default(), ModSelection::None)
    };
    let combo = if let Some(combo) = arg_parser.get_combo() {
        match u32::from_str(&combo) {
            Ok(val) => val,
            Err(_) => {
                msg.channel_id.say(
                    &ctx.http,
                    "Could not parse given combo, try a non-negative integer",
                )?;
                return Ok(());
            }
        }
    } else {
        0
    };
    let acc = if let Some(acc) = arg_parser.get_acc() {
        match f32::from_str(&acc) {
            Ok(val) => val,
            Err(_) => {
                msg.channel_id.say(
                    &ctx.http,
                    "Could not parse given accuracy, \
                     try a decimal number between 0 and 100",
                )?;
                return Ok(());
            }
        }
    } else {
        0.0
    };
    let grade = if let Some(grade) = arg_parser.get_grade() {
        match Grade::try_from(grade.as_ref()) {
            Ok(grade) => Some(grade),
            Err(_) => {
                msg.channel_id.say(
                    &ctx.http,
                    "Could not parse given grade, \
                     try SS, S, A, B, C, D, or F",
                )?;
                return Ok(());
            }
        }
    } else {
        None
    };
    let name: String = if let Some(name) = arg_parser.get_name() {
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
    let user_args = UserArgs::with_username(&name).mode(mode);
    let best_args = UserBestArgs::with_username(&name).mode(mode).limit(100);
    let (user_req, best_req): (OsuRequest<User>, OsuRequest<Score>) = {
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let user_req = osu.create_request(OsuArgs::Users(user_args));
        let best_req = osu.create_request(OsuArgs::Best(best_args));
        (user_req, best_req)
    };
    let mut rt = Runtime::new().unwrap();

    // Retrieve the user and its top scores
    let res = rt.block_on(async {
        let users = user_req
            .queue()
            .await
            .or_else(|e| Err(CommandError(format!("Error while retrieving Users: {}", e))))?;
        let scores = best_req.queue().await.or_else(|e| {
            Err(CommandError(format!(
                "Error while retrieving UserBest: {}",
                e
            )))
        })?;
        Ok((users, scores))
    });
    let (user, scores): (User, Vec<Score>) = match res {
        Ok((mut users, scores)) => {
            let user = match users.pop() {
                Some(user) => user,
                None => {
                    msg.channel_id
                        .say(&ctx.http, format!("User {} was not found", name))?;
                    return Ok(());
                }
            };
            (user, scores)
        }
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
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
                ModSelection::Exact => mods == s.enabled_mods,
                ModSelection::Includes => mods.iter().all(|m| s.enabled_mods.contains(&m)),
                ModSelection::Excludes => mods.iter().all(|m| !s.enabled_mods.contains(&m)),
            };
            if !mod_bool {
                return false;
            }
            let acc_bool = if acc > 0.0 {
                s.get_accuracy(mode) >= acc
            } else {
                true
            };
            acc_bool && s.max_combo >= combo
        })
        .collect();
    let amount = scores_indices.len();
    scores_indices = scores_indices[..amount.min(5)].to_vec();
    scores_indices.iter_mut().for_each(|(i, _)| *i += 1);

    // Retrieving each score's beatmap
    let res = rt.block_on(async move {
        let mut tuples = Vec::with_capacity(scores_indices.len());
        for (i, score) in scores_indices.into_iter() {
            let map = score
                .beatmap
                .as_ref()
                .unwrap()
                .get(mode)
                .await
                .or_else(|e| {
                    Err(CommandError(format!(
                        "Error while retrieving LazilyLoaded<Beatmap> of best score: {}",
                        e
                    )))
                })?;
            tuples.push((i, score, map));
        }
        Ok(tuples)
    });
    let scores_data = match res {
        Ok(tuples) => tuples,
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
        }
    };

    // Accumulate all necessary data
    let data = MapMultiData::new(user, scores_data, mode, ctx.cache.clone());

    // Creating the embed
    let embed = BotEmbed::UserMapMulti(data);
    let _ = msg.channel_id.send_message(&ctx.http, |m| {
        let mut content = format!("Found {} top scores with the specified properties", amount);
        if amount > 5 {
            content.push_str(", here's the top 5 of them:");
        } else {
            content.push(':');
        }
        m.content(content).embed(|e| embed.create(e))
    });
    Ok(())
}

#[command]
#[description = "Display a user's top plays"]
#[usage = "badewanne3"]
#[aliases("topscores", "osutop")]
pub fn top(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::STD, ctx, msg, args)
}

#[command]
#[description = "Display a user's top mania plays"]
#[usage = "badewanne3"]
#[aliases("topm")]
pub fn topmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::MNA, ctx, msg, args)
}

#[command]
#[description = "Display a user's top taiko plays"]
#[usage = "badewanne3"]
#[aliases("topt")]
pub fn toptaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::TKO, ctx, msg, args)
}

#[command]
#[description = "Display a user's top ctb plays"]
#[usage = "badewanne3"]
#[aliases("topc")]
pub fn topctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::CTB, ctx, msg, args)
}
