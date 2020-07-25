use super::require_link;
use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, RecentEmbed},
    pagination::{Pagination, RecentPagination},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, Context,
};

use rosu::{
    backend::requests::{RecentRequest, UserRequest},
    models::{
        ApprovalStatus::{Approved, Loved, Qualified, Ranked},
        GameMode,
    },
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use twilight::model::channel::Message;

#[allow(clippy::cognitive_complexity)]
async fn recent_send(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = NameArgs::new(args);
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return require_link(&ctx, msg).await,
    };

    // Retrieve the recent scores
    let request = RecentRequest::with_username(&name).mode(mode).limit(50);
    let osu = &ctx.clients.osu;
    let scores = match request.queue(osu).await {
        Ok(scores) => scores,
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why.into());
        }
    };
    if scores.is_empty() {
        let content = format!("No recent plays found for user `{}`", name);
        return msg.respond(&ctx, content).await;
    }

    // Retrieving the score's user
    let req = UserRequest::with_username(&name).mode(mode);
    let user = match req.queue_single(osu).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let content = format!("User `{}` was not found", name);
            return msg.respond(&ctx, content).await;
        }
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why.into());
        }
    };

    // Get all relevant maps from the database
    let mut map_ids: HashSet<u32> = scores.iter().filter_map(|s| s.beatmap_id).collect();
    let mut maps = {
        let dedubed_ids: Vec<u32> = map_ids.iter().copied().collect();
        let map_result = ctx.clients.psql.get_beatmaps(&dedubed_ids).await;
        match map_result {
            Ok(maps) => maps,
            Err(why) => {
                warn!("Error while retrieving maps from DB: {}", why);
                HashMap::default()
            }
        }
    };

    // Memoize which maps are already in the DB
    map_ids.retain(|id| maps.contains_key(&id));

    let first_score = scores.first().unwrap();
    let first_id = first_score.beatmap_id.unwrap();
    #[allow(clippy::map_entry)]
    if !maps.contains_key(&first_id) {
        let map = match first_score.get_beatmap(osu).await {
            Ok(map) => map,
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        };
        maps.insert(first_id, map);
    }

    // Retrieving the user's top 100 and the map's global top 50
    let best = match user.get_top_scores(osu, 100, mode).await {
        Ok(scores) => scores,
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why.into());
        }
    };
    let first_map = maps.get(&first_id).unwrap();
    let mut global = HashMap::with_capacity(50);
    match first_map.approval_status {
        Ranked | Loved | Qualified | Approved => {
            match first_map.get_global_leaderboard(osu, 50).await {
                Ok(scores) => {
                    global.insert(first_map.beatmap_id, scores);
                }
                Err(why) => {
                    msg.respond(&ctx, OSU_API_ISSUE).await?;
                    return Err(why.into());
                }
            }
        }
        _ => {}
    }

    // Accumulate all necessary data
    let tries = scores
        .iter()
        .take_while(|s| {
            s.beatmap_id.unwrap() == first_id && s.enabled_mods == first_score.enabled_mods
        })
        .count();
    let global_scores = global.get(&first_map.beatmap_id).unwrap();
    let data = match RecentEmbed::new(
        &user,
        first_score,
        first_map,
        &best,
        global_scores,
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

    // Creating the embed
    let embed = data.build().build();
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .content(format!("Try #{}", tries))?
        .embed(embed)?
        .await?;

    // Skip pagination if too few entries
    if scores.len() <= 1 {
        response.reaction_delete(&ctx, msg.author.id);
        return Ok(());
    }

    // Pagination
    let pagination = RecentPagination::new(
        ctx.clone(),
        response,
        user,
        scores,
        maps,
        best,
        global,
        map_ids,
        data,
    );
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 90).await {
            warn!("Pagination error: {}", why)
        }
    });
    Ok(())
}

#[command]
#[short_desc("Display a user's most recent play")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("r", "rs")]
pub async fn recent(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_send(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's most recent mania play")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rm")]
pub async fn recentmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_send(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's most recent taiko play")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rt")]
pub async fn recenttaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_send(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's most recent ctb play")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rc")]
pub async fn recentctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_send(GameMode::CTB, ctx, msg, args).await
}
