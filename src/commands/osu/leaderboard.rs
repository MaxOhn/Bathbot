use crate::{
    arguments::{MapModArgs, ModSelection},
    database::MySQL,
    embeds::BasicEmbedData,
    scraper::{Scraper, ScraperScore},
    util::{
        discord,
        globals::{AVATAR_URL, OSU_API_ISSUE},
    },
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::BeatmapRequest,
    models::{
        ApprovalStatus::{Loved, Ranked},
        Beatmap, GameMods,
    },
};
use serenity::{
    cache::CacheRwLock,
    collector::{ReactionAction, ReactionCollectorBuilder},
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::channel::{Message, ReactionType},
    prelude::{Context, RwLock, ShareMap},
};
use std::{sync::Arc, time::Duration};

#[allow(clippy::cognitive_complexity)]
async fn leaderboard_send(
    national: bool,
    ctx: &mut Context,
    msg: &Message,
    args: Args,
) -> CommandResult {
    let init_name = {
        let data = ctx.data.read().await;
        let links = data
            .get::<DiscordLinks>()
            .expect("Could not get DiscordLinks");
        links.get(msg.author.id.as_u64()).cloned()
    };
    let args = MapModArgs::new(args);
    let map_id = if let Some(id) = args.map_id {
        id
    } else {
        let msgs = msg
            .channel_id
            .messages(&ctx.http, |retriever| retriever.limit(50))
            .await?;
        match discord::map_id_from_history(msgs, ctx.cache.clone()).await {
            Some(id) => id,
            None => {
                msg.channel_id
                    .say(
                        &ctx.http,
                        "No beatmap specified and none found in recent channel history. \
                     Try specifying a map either by url to the map, or just by map id.",
                    )
                    .await?;
                return Ok(());
            }
        }
    };
    let (mods, selection) = args
        .mods
        .unwrap_or_else(|| (GameMods::default(), ModSelection::None));

    // Retrieving the beatmap
    let (map_to_db, map) = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        match mysql.get_beatmap(map_id) {
            Ok(map) => (false, map),
            Err(_) => {
                let map_req = BeatmapRequest::new().map_id(map_id);
                let osu = data.get::<Osu>().expect("Could not get osu client");
                let map = match map_req.queue_single(&osu).await {
                    Ok(result) => match result {
                        Some(map) => map,
                        None => {
                            msg.channel_id
                                .say(
                                    &ctx.http,
                                    format!(
                                        "Could not find beatmap with id `{}`. \
                                Did you give me a mapset id instead of a map id?",
                                        map_id
                                    ),
                                )
                                .await?;
                            return Ok(());
                        }
                    },
                    Err(why) => {
                        msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                        return Err(CommandError::from(why.to_string()));
                    }
                };
                (
                    map.approval_status == Ranked || map.approval_status == Loved,
                    map,
                )
            }
        }
    };

    // Retrieve the map's leaderboard
    let scores = {
        let data = ctx.data.read().await;
        let scraper = data.get::<Scraper>().expect("Could not get Scraper");
        let scores_future = scraper.get_leaderboard(
            map_id,
            national,
            match selection {
                ModSelection::Excludes | ModSelection::None => None,
                _ => Some(&mods),
            },
        );
        match scores_future.await {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };
    let amount = scores.len();

    // Accumulate all necessary data
    let map_copy = if map_to_db { Some(map.clone()) } else { None };
    let author_icon = scores
        .first()
        .map(|s| format!("{}{}", AVATAR_URL, s.user_id));
    let data = match BasicEmbedData::create_leaderboard(
        &init_name.as_deref(),
        &map,
        if scores.is_empty() {
            None
        } else {
            Some(scores.iter().take(10))
        },
        &author_icon,
        0,
        &ctx,
    )
    .await
    {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id
                .say(
                    &ctx.http,
                    "Some issue while calculating leaderboard data, blame bade",
                )
                .await?;
            return Err(CommandError::from(why.to_string()));
        }
    };

    // Sending the embed
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            let mut content = format!(
                "I found {} scores with the specified mods on the map's leaderboard",
                amount
            );
            if amount > 10 {
                content.push_str(", here's the top 10 of them:");
            } else {
                content.push(':');
            }
            m.content(content).embed(|e| data.build(e))
        })
        .await;

    // Add map to database if its not in already
    if let Some(map) = map_copy {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.insert_beatmap(&map) {
            warn!("Could not add map of recent command to database: {}", why);
        }
    }
    let mut response = response?;

    // Collect reactions of author on the response
    let mut collector = ReactionCollectorBuilder::new(&ctx)
        .author_id(msg.author.id)
        .message_id(response.id)
        .timeout(Duration::from_secs(60))
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
        let author_name = init_name.as_deref();
        while let Some(reaction) = collector.receive_one().await {
            if let ReactionAction::Added(reaction) = &*reaction {
                if let ReactionType::Unicode(reaction_name) = &reaction.emoji {
                    if reaction_name.as_str() == "❌" {
                        response.delete((&cache, &*http)).await?;
                    } else if !scores.is_empty() {
                        let reaction_data = reaction_data(
                            reaction_name.as_str(),
                            &mut idx,
                            &map,
                            &scores,
                            &author_name,
                            &author_icon,
                            &cache,
                            &data,
                        );
                        match reaction_data.await {
                            ReactionData::None => {}
                            ReactionData::Data(data) => {
                                response
                                    .edit((&cache, &*http), |m| m.embed(|e| data.build(e)))
                                    .await?
                            }
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
    None,
}

#[allow(clippy::too_many_arguments)]
async fn reaction_data(
    reaction: &str,
    idx: &mut usize,
    map: &Beatmap,
    scores: &[ScraperScore],
    author_name: &Option<&str>,
    author_icon: &Option<String>,
    cache: &CacheRwLock,
    data: &Arc<RwLock<ShareMap>>,
) -> ReactionData {
    let amount = scores.len();
    match reaction {
        "⏮️" => {
            if *idx > 0 {
                *idx = 0;
                BasicEmbedData::create_leaderboard(
                    author_name,
                    map,
                    Some(scores.iter().take(10)),
                    author_icon,
                    *idx,
                    (cache, data),
                )
                .await
                .map(|data| ReactionData::Data(Box::new(data)))
                .unwrap_or_else(|why| {
                    warn!(
                        "Error editing leaderboard data at idx {}/{}: {}",
                        idx, amount, why
                    );
                    ReactionData::None
                })
            } else {
                ReactionData::None
            }
        }
        "⏪" => {
            if *idx > 0 {
                *idx = idx.saturating_sub(10);
                BasicEmbedData::create_leaderboard(
                    author_name,
                    map,
                    Some(scores.iter().skip(*idx).take(10)),
                    author_icon,
                    *idx,
                    (cache, data),
                )
                .await
                .map(|data| ReactionData::Data(Box::new(data)))
                .unwrap_or_else(|why| {
                    warn!(
                        "Error editing leaderboard data at idx {}/{}: {}",
                        idx, amount, why
                    );
                    ReactionData::None
                })
            } else {
                ReactionData::None
            }
        }
        "⏩" => {
            let limit = amount.saturating_sub(10);
            if *idx < limit {
                *idx = limit.min(*idx + 10);
                BasicEmbedData::create_leaderboard(
                    author_name,
                    map,
                    Some(scores.iter().skip(*idx).take(10)),
                    author_icon,
                    *idx,
                    (cache, data),
                )
                .await
                .map(|data| ReactionData::Data(Box::new(data)))
                .unwrap_or_else(|why| {
                    warn!(
                        "Error editing leaderboard data at idx {}/{}: {}",
                        idx, amount, why
                    );
                    ReactionData::None
                })
            } else {
                ReactionData::None
            }
        }
        "⏭️" => {
            let limit = amount.saturating_sub(10);
            if *idx < limit {
                *idx = limit;
                BasicEmbedData::create_leaderboard(
                    author_name,
                    map,
                    Some(scores.iter().skip(*idx).take(10)),
                    author_icon,
                    *idx,
                    (cache, data),
                )
                .await
                .map(|data| ReactionData::Data(Box::new(data)))
                .unwrap_or_else(|why| {
                    warn!(
                        "Error editing leaderboard data at idx {}/{}: {}",
                        idx, amount, why
                    );
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
#[description = "Display the national leaderboard of a given map. \
                 If no map is given, I will choose the last map \
                 I can find in my embeds of this channel"]
#[usage = "[map url / map id]"]
#[example = "2240404"]
#[example = "https://osu.ppy.sh/beatmapsets/902425#osu/2240404"]
#[aliases("lb")]
pub async fn leaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    leaderboard_send(true, ctx, msg, args).await
}

#[command]
#[description = "Display the global leaderboard of a given map. \
                 If no map is given, I will choose the last map \
                 I can find in my embeds of this channel"]
#[usage = "[map url / map id]"]
#[example = "2240404"]
#[example = "https://osu.ppy.sh/beatmapsets/902425#osu/2240404"]
#[aliases("glb")]
pub async fn globalleaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    leaderboard_send(false, ctx, msg, args).await
}
