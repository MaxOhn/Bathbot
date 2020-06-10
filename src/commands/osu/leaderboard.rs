use crate::{
    arguments::{MapModArgs, ModSelection},
    database::MySQL,
    embeds::BasicEmbedData,
    pagination::{Pagination, ReactionData},
    scraper::Scraper,
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
        GameMods,
    },
};
use serenity::{
    collector::ReactionAction,
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::channel::{Message, ReactionType},
    prelude::Context,
};
use std::{convert::TryFrom, sync::Arc, time::Duration};
use tokio::stream::StreamExt;

#[allow(clippy::cognitive_complexity)]
async fn leaderboard_send(
    national: bool,
    ctx: &Context,
    msg: &Message,
    args: Args,
) -> CommandResult {
    let author_name = {
        let data = ctx.data.read().await;
        data.get::<DiscordLinks>()
            .and_then(|links| links.get(msg.author.id.as_u64()).cloned())
    };
    let args = MapModArgs::new(args);
    let map_id = if let Some(id) = args.map_id {
        id
    } else {
        let msgs = msg
            .channel_id
            .messages(&ctx.http, |retriever| retriever.limit(50))
            .await?;
        match discord::map_id_from_history(msgs, &ctx.cache).await {
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
        let mysql = data.get::<MySQL>().unwrap();
        match mysql.get_beatmap(map_id) {
            Ok(map) => (false, map),
            Err(_) => {
                let map_req = BeatmapRequest::new().map_id(map_id);
                let osu = data.get::<Osu>().unwrap();
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
        let scraper = data.get::<Scraper>().unwrap();
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
    let first_place_icon = scores
        .first()
        .map(|s| format!("{}{}", AVATAR_URL, s.user_id));
    let data = match BasicEmbedData::create_leaderboard(
        &author_name.as_deref(),
        &map,
        if scores.is_empty() {
            None
        } else {
            Some(scores.iter().take(10))
        },
        &first_place_icon,
        0,
        ctx,
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
        let mysql = data.get::<MySQL>().unwrap();
        if let Err(why) = mysql.insert_beatmap(&map) {
            warn!("Could not add map of recent command to DB: {}", why);
        }
    }
    let mut response = response?;
    if scores.is_empty() {
        discord::reaction_deletion(&ctx, response, msg.author.id).await;
        return Ok(());
    }

    // Collect reactions of author on the response
    let mut collector = response
        .await_reactions(&ctx)
        .timeout(Duration::from_secs(60))
        .author_id(msg.author.id)
        .await;

    // Add initial reactions
    let reactions = ["⏮️", "⏪", "⏩", "⏭️"];
    for &reaction in reactions.iter() {
        let reaction_type = ReactionType::try_from(reaction).unwrap();
        response.react(&ctx.http, reaction_type).await?;
    }

    // Check if the author wants to edit the response
    let http = Arc::clone(&ctx.http);
    let cache = Arc::clone(&ctx.cache);
    let data = Arc::clone(&ctx.data);
    tokio::spawn(async move {
        let mut pagination = Pagination::leaderboard(
            map,
            scores,
            author_name,
            first_place_icon,
            Arc::clone(&cache),
            data,
        );
        while let Some(reaction) = collector.next().await {
            if let ReactionAction::Added(reaction) = &*reaction {
                if let ReactionType::Unicode(reaction) = &reaction.emoji {
                    match pagination.next_reaction(reaction.as_str()).await {
                        Ok(data) => match data {
                            ReactionData::Delete => response.delete((&cache, &*http)).await?,
                            ReactionData::None => {}
                            _ => {
                                response
                                    .edit((&cache, &*http), |m| m.embed(|e| data.build(e)))
                                    .await?
                            }
                        },
                        Err(why) => warn!("Error while using paginator for leaderboard: {}", why),
                    }
                }
            }
        }
        for &reaction in reactions.iter() {
            let reaction_type = ReactionType::try_from(reaction).unwrap();
            response
                .channel_id
                .delete_reaction(&http, response.id, None, reaction_type)
                .await?;
        }
        Ok::<_, serenity::Error>(())
    });
    Ok(())
}

#[command]
#[description = "Display the belgian leaderboard of a given map. \
                 If no map is given, I will choose the last map \
                 I can find in my embeds of this channel"]
#[usage = "[map url / map id]"]
#[example = "2240404"]
#[example = "https://osu.ppy.sh/beatmapsets/902425#osu/2240404"]
#[aliases("lb")]
pub async fn leaderboard(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
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
pub async fn globalleaderboard(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    leaderboard_send(false, ctx, msg, args).await
}
