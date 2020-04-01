use crate::{
    arguments::NamePassArgs,
    database::MySQL,
    embeds::BasicEmbedData,
    util::{discord, globals::OSU_API_ISSUE},
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::{RecentRequest, UserRequest},
    models::{
        ApprovalStatus::{Loved, Ranked},
        Beatmap, GameMode, Grade,
    },
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::collections::{HashMap, HashSet};

async fn recentlist_send(
    mode: GameMode,
    ctx: &mut Context,
    msg: &Message,
    args: Args,
) -> CommandResult {
    let args = NamePassArgs::new(args);
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
    let pass = args.pass;

    // Retrieve the recent scores
    let scores = {
        let request = RecentRequest::with_username(&name).mode(mode).limit(50);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        match request.queue(osu).await {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };
    if scores.is_empty() {
        msg.channel_id
            .say(
                &ctx.http,
                format!("No recent plays found for user `{}`", name),
            )
            .await?;
        return Ok(());
    };
    let scores: Vec<_> = scores
        .into_iter()
        .filter(|s| !pass || s.grade != Grade::F)
        .take(5)
        .collect();
    if pass && scores.is_empty() {
        msg.channel_id
            .say(&ctx.http, format!("User `{}` has no recent passes", name))
            .await?;
        return Ok(());
    };

    // Retrieving the score's user
    let user = {
        let req = UserRequest::with_username(&name).mode(mode);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        match req.queue_single(&osu).await {
            Ok(Some(u)) => u,
            Ok(None) => {
                msg.channel_id
                    .say(&ctx.http, format!("User `{}` was not found", name))
                    .await?;
                return Ok(());
            }
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };

    // Get all relevant maps from the database
    let map_ids: Vec<u32> = scores
        .iter()
        .map(|s| s.beatmap_id.unwrap())
        .collect::<HashSet<_>>() // Remove all duplicates
        .drain()
        .collect();
    let maps = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql
            .get_beatmaps(&map_ids)
            .unwrap_or_else(|_| HashMap::default())
    };
    let map_ids: HashSet<_> = map_ids.into_iter().collect();
    info!(
        "Found {}/{} beatmaps in the database",
        maps.len(),
        map_ids.len()
    );

    // Retrieving all missing beatmaps
    let res = {
        let mut maps = maps;
        let mut missing_maps = HashSet::with_capacity(maps.len());
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        for score in scores.iter() {
            let map_id = score.beatmap_id.unwrap();
            if !maps.contains_key(&map_id) {
                let map = score.get_beatmap(osu).await.or_else(|e| {
                    Err(CommandError(format!(
                        "Error while retrieving Beatmap of score: {}",
                        e
                    )))
                })?;
                // Only mark maps as missing if they're ranked or loved
                match map.approval_status {
                    Ranked | Loved => {
                        missing_maps.insert(map.beatmap_id);
                    }
                    _ => {}
                }
                maps.insert(map.beatmap_id, map);
            };
        }
        Ok((maps, missing_maps))
    };
    let (missing_maps, maps): (Option<Vec<Beatmap>>, HashMap<_, _>) = match res {
        Ok((maps, missing_maps)) => {
            if missing_maps.is_empty() {
                (None, maps)
            } else {
                (
                    Some(
                        maps.iter()
                            .filter(|(id, _)| missing_maps.contains(id))
                            .map(|(_, map)| map.clone())
                            .collect(),
                    ),
                    maps,
                )
            }
        }
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
            return Err(why);
        }
    };

    // Retrieving the user's top 100 and the map's global top 50
    let best = {
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        match user.get_top_scores(osu, 100, mode).await {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };

    // Accumulate all necessary data
    let content = if scores.len() == 1 {
        let mut content = String::from("Most recent ");
        if pass {
            content.push_str("pass");
        } else {
            content.push_str("play");
        }
        content.push(':');
        content
    } else {
        format!(
            "{} most recent {}:",
            scores.len(),
            if pass { "passes" } else { "plays" }
        )
    };
    let data = match BasicEmbedData::create_recentlist(user, scores, maps, best, mode, &ctx).await {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id
                .say(
                    &ctx.http,
                    "Some issue while calculating recent data, blame bade",
                )
                .await?;
            return Err(CommandError::from(why.to_string()));
        }
    };

    // Creating the embed
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.content(content).embed(|e| data.build(e)))
        .await?;

    // Add missing maps to database
    if let Some(maps) = missing_maps {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.insert_beatmaps(maps) {
            warn!(
                "Could not add missing maps of recentlist command to database: {}",
                why
            );
        }
    }

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone()).await;
    Ok(())
}

#[command]
#[description = "Display a user's 5 most recent plays. If `-pass` is specified, \
I will only consider passes"]
#[usage = "[username] [-pass]"]
#[example = "badewanne3 -pass"]
#[aliases("rl")]
pub async fn recentlist(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recentlist_send(GameMode::STD, ctx, msg, args).await
}

#[command]
#[description = "Display a user's 5 most recent mania plays. If `-pass` is specified, \
I will only consider passes"]
#[usage = "[username] [-pass]"]
#[example = "badewanne3 -pass"]
#[aliases("rlm")]
pub async fn recentlistmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recentlist_send(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[description = "Display a user's 5 most recent taiko plays. If `-pass` is specified, \
I will only consider passes"]
#[usage = "[username] [-pass]"]
#[example = "badewanne3 -pass"]
#[aliases("rlt")]
pub async fn recentlisttaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recentlist_send(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[description = "Display a user's 5 most recent ctb plays. If `-pass` is specified, \
I will only consider passes"]
#[usage = "[username] [-pass]"]
#[example = "badewanne3 -pass"]
#[aliases("rlc")]
pub async fn recentlistctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recentlist_send(GameMode::CTB, ctx, msg, args).await
}
