use crate::{
    arguments::NameArgs,
    database::MySQL,
    embeds::{EmbedData, RecentEmbed},
    pagination::{Pagination, RecentPagination},
    util::{globals::OSU_API_ISSUE, MessageExt},
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::{RecentRequest, UserRequest},
    models::{
        ApprovalStatus::{Approved, Loved, Qualified, Ranked},
        GameMode,
    },
};
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::Message,
    prelude::Context,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

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
                        ctx,
                        "Either specify an osu name or link your discord \
                        to an osu profile via `<link osuname`",
                    )
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
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
                msg.channel_id
                    .say(ctx, OSU_API_ISSUE)
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(why.to_string().into());
            }
        }
    };
    if scores.is_empty() {
        msg.channel_id
            .say(ctx, format!("No recent plays found for user `{}`", name))
            .await?
            .reaction_delete(ctx, msg.author.id)
            .await;
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
                msg.channel_id
                    .say(ctx, OSU_API_ISSUE)
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(why.to_string().into());
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
            .await
            .unwrap_or_else(|_| HashMap::default())
    };

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
                    msg.channel_id
                        .say(ctx, OSU_API_ISSUE)
                        .await?
                        .reaction_delete(ctx, msg.author.id)
                        .await;
                    return Err(why.to_string().into());
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
                msg.channel_id
                    .say(ctx, OSU_API_ISSUE)
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(why.to_string().into());
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
                        msg.channel_id
                            .say(ctx, OSU_API_ISSUE)
                            .await?
                            .reaction_delete(ctx, msg.author.id)
                            .await;
                        return Err(why.to_string().into());
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
    let global_scores = global.get(&first_map.beatmap_id).unwrap();
    let embed_data =
        match RecentEmbed::new(&user, first_score, first_map, &best, global_scores, ctx).await {
            Ok(data) => data,
            Err(why) => {
                msg.channel_id
                    .say(ctx, "Some issue while calculating recent data, blame bade")
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(why.to_string().into());
            }
        };

    // Creating the embed
    let resp = msg
        .channel_id
        .send_message(ctx, |m| {
            m.content(format!("Try #{}", tries))
                .embed(|e| embed_data.build(e))
        })
        .await?;

    // Skip pagination if too few entries
    if scores.len() <= 1 {
        resp.reaction_delete(ctx, msg.author.id).await;
        return Ok(());
    }

    // Pagination
    let pagination = RecentPagination::new(
        ctx,
        resp,
        msg.author.id,
        user,
        scores,
        maps,
        best,
        global,
        map_ids,
        embed_data,
    )
    .await;
    let cache = Arc::clone(&ctx.cache);
    let http = Arc::clone(&ctx.http);
    tokio::spawn(async move {
        if let Err(why) = pagination.start(cache, http).await {
            warn!("Pagination error: {}", why)
        }
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
