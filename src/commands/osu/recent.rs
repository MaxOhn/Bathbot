use crate::{
    arguments::NameArgs,
    database::MySQL,
    embeds::RecentData,
    pagination::{Pagination, ReactionData},
    util::globals::OSU_API_ISSUE,
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::{RecentRequest, UserRequest},
    models::{
        ApprovalStatus::{Approved, Loved, Qualified, Ranked},
        Beatmap, GameMode,
    },
};
use serenity::{
    collector::ReactionAction,
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::channel::{Message, ReactionType},
    prelude::Context,
};
use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
    sync::Arc,
};
use tokio::stream::StreamExt;
use tokio::time::Duration;

#[allow(clippy::cognitive_complexity)]
async fn recent_send(mode: GameMode, ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let args = NameArgs::new(args);
    let name = if let Some(name) = args.name {
        name
    } else {
        let data = ctx.data.read().await;
        let links = data.get::<DiscordLinks>().unwrap();
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

    // Retrieve the recent scores
    let scores = {
        let request = RecentRequest::with_username(&name).mode(mode).limit(50);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
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
    }

    // Retrieving the score's user
    let user = {
        let req = UserRequest::with_username(&name).mode(mode);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        match req.queue_single(&osu).await {
            Ok(Some(u)) => u,
            Ok(None) => unreachable!(),
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };

    // Get all relevant maps from the database
    let mut map_ids: HashSet<u32> = scores.iter().map(|s| s.beatmap_id.unwrap()).collect();
    let mut maps = {
        let dedubed_ids: Vec<u32> = map_ids.iter().copied().collect();
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        mysql
            .get_beatmaps(&dedubed_ids)
            .unwrap_or_else(|_| HashMap::default())
    };
    // debug!("Found {}/{} beatmaps in DB", maps.len(), map_ids.len());

    // Memoize which maps are already in the DB
    map_ids.retain(|id| maps.contains_key(&id));

    let first_score = scores.first().unwrap();
    let first_id = first_score.beatmap_id.unwrap();
    #[allow(clippy::map_entry)]
    {
        if !maps.contains_key(&first_id) {
            let data = ctx.data.read().await;
            let osu = data.get::<Osu>().unwrap();
            let map = match first_score.get_beatmap(osu).await {
                Ok(map) => map,
                Err(why) => {
                    msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                    return Err(CommandError::from(why.to_string()));
                }
            };
            maps.insert(first_id, map);
        }
    }

    // Retrieving the user's top 100 and the map's global top 50
    let best = {
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        match user.get_top_scores(osu, 100, mode).await {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };
    let first_map = maps.get(&first_id).unwrap();
    let mut global = HashMap::with_capacity(50);
    {
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        match first_map.approval_status {
            Ranked | Loved | Qualified | Approved => {
                match first_map.get_global_leaderboard(osu, 50).await {
                    Ok(scores) => {
                        global.insert(first_map.beatmap_id, scores);
                    }
                    Err(why) => {
                        msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                        return Err(CommandError::from(why.to_string()));
                    }
                }
            }
            _ => {}
        }
    }

    // Accumulate all necessary data
    let tries = scores
        .iter()
        .take_while(|s| {
            s.beatmap_id.unwrap() == first_id && s.enabled_mods == first_score.enabled_mods
        })
        .count();
    let mut embed_data = match RecentData::new(
        &user,
        first_score,
        first_map,
        &best,
        global.get(&first_map.beatmap_id).unwrap(),
        ctx,
    )
    .await
    {
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
    let mut response = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.content(format!("Try #{}", tries))
                .embed(|e| embed_data.build(e))
        })
        .await?;

    // Collect reactions of author on the response
    let mut collector = response
        .await_reactions(&ctx)
        .timeout(Duration::from_secs(45))
        .author_id(msg.author.id)
        .await;

    // Add initial reactions
    let reactions = ["⏮️", "⏪", "◀️", "▶️", "⏩", "⏭️"];
    for &reaction in reactions.iter() {
        let reaction_type = ReactionType::try_from(reaction).unwrap();
        response.react(&ctx.http, reaction_type).await?;
    }

    // Check if the author wants to edit the response
    let http = Arc::clone(&ctx.http);
    let cache = ctx.cache.clone();
    let data = Arc::clone(&ctx.data);
    tokio::spawn(async move {
        let mut pagination = Pagination::recent(
            user,
            scores,
            maps,
            best,
            global,
            cache.clone(),
            Arc::clone(&data),
        );
        while let Some(reaction) = collector.next().await {
            if let ReactionAction::Added(reaction) = &*reaction {
                if let ReactionType::Unicode(reaction) = &reaction.emoji {
                    match pagination.next_reaction(reaction.as_str()).await {
                        Ok(data) => match data {
                            ReactionData::Delete => response.delete((&cache, &*http)).await?,
                            ReactionData::None => {}
                            _ => {
                                let content = format!("Recent score #{}", pagination.index + 1);
                                embed_data = data.recent_data();
                                response
                                    .edit((&cache, &*http), |m| {
                                        m.content(content).embed(|e| embed_data.build(e))
                                    })
                                    .await?
                            }
                        },
                        Err(why) => warn!("Error while using paginator for recent: {}", why),
                    }
                }
            }
        }

        // Remove initial reactions
        for &reaction in reactions.iter() {
            let reaction_type = ReactionType::try_from(reaction).unwrap();
            response
                .channel_id
                .delete_reaction(&http, response.id, None, reaction_type)
                .await?;
        }

        // Minimize embed
        response
            .edit((&cache, &*http), |m| m.embed(|e| embed_data.minimize(e)))
            .await?;

        // Put missing maps into DB
        let maps = pagination.recent_maps();
        if maps.len() > map_ids.len() {
            let maps: Vec<Beatmap> = maps
                .into_iter()
                .filter(|(id, _)| !map_ids.contains(&id))
                .map(|(_, map)| map)
                .collect();
            let data = data.read().await;
            let mysql = data.get::<MySQL>().unwrap();
            if let Err(why) = mysql.insert_beatmaps(maps) {
                warn!("Error while adding maps to DB: {}", why);
            }
        }
        Ok::<_, serenity::Error>(())
    });
    Ok(())
}

#[command]
#[description = "Display a user's most recent play"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("r", "rs")]
pub async fn recent(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::STD, ctx, msg, args).await
}

#[command]
#[description = "Display a user's most recent mania play"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("rm")]
pub async fn recentmania(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[description = "Display a user's most recent taiko play"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("rt")]
pub async fn recenttaiko(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[description = "Display a user's most recent ctb play"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("rc")]
pub async fn recentctb(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::CTB, ctx, msg, args).await
}
