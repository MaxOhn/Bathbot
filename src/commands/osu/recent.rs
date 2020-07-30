use super::require_link;
use crate::{
    arguments::{Args, NameArgs},
    bail,
    embeds::{EmbedData, RecentEmbed},
    pagination::{Pagination, RecentPagination},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, Context,
};

use rosu::{
    backend::requests::RecentRequest,
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
async fn recent_main(
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

    // Retrieve the user and their recent scores
    let join_result = tokio::try_join!(
        ctx.osu_user(&name, mode),
        RecentRequest::with_username(&name)
            .mode(mode)
            .limit(50)
            .queue(ctx.osu())
    );
    let (user, scores) = match join_result {
        Ok((user, scores)) => {
            if scores.is_empty() {
                let content = format!("No recent plays found for user `{}`", name);
                return msg.error(&ctx, content).await;
            } else if let Some(user) = user {
                (user, scores)
            } else {
                let content = format!("User `{}` was not found", name);
                return msg.error(&ctx, content).await;
            }
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };

    // Get all relevant maps from the database
    let mut map_ids: HashSet<u32> = scores.iter().filter_map(|s| s.beatmap_id).collect();
    let mut maps = {
        let dedubed_ids: Vec<u32> = map_ids.iter().copied().collect();
        let map_result = ctx.psql().get_beatmaps(&dedubed_ids).await;
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

    // Prepare retrieval of the first map, the user's top 100, and the map's global top 50
    let first_score = scores.first().unwrap();
    let first_id = first_score.beatmap_id.unwrap();
    let map_fut = async {
        if !maps.contains_key(&first_id) {
            Some(first_score.get_beatmap(ctx.osu()).await)
        } else {
            None
        }
    };
    let globals_fut = async {
        let first_map = maps.get(&first_id).unwrap();
        match first_map.approval_status {
            Ranked | Loved | Qualified | Approved => {
                Some(first_map.get_global_leaderboard(ctx.osu(), 50).await)
            }
            _ => None,
        }
    };

    // Retrieve and process responses
    let (map_result, best_result, globals_result) = tokio::join!(
        map_fut,
        user.get_top_scores(ctx.osu(), 100, mode),
        globals_fut
    );
    match map_result {
        None => {}
        Some(Ok(map)) => {
            maps.insert(first_id, map);
        }
        Some(Err(why)) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    }
    let best = match best_result {
        Ok(scores) => scores,
        Err(why) => {
            warn!("Error while getting top scores: {}", why);
            Vec::new()
        }
    };
    let mut global = HashMap::with_capacity(50);
    match globals_result {
        None => {}
        Some(Ok(scores)) => {
            global.insert(first_id, scores);
        }
        Some(Err(why)) => warn!("Error while getting global scores: {}", why),
    }

    // Accumulate all necessary data
    let tries = scores
        .iter()
        .take_while(|s| {
            s.beatmap_id.unwrap() == first_id && s.enabled_mods == first_score.enabled_mods
        })
        .count();
    let global_scores = global.get(&first_id).unwrap();
    let first_map = maps.get(&first_id).unwrap();
    let data =
        match RecentEmbed::new(&ctx, &user, first_score, first_map, &best, global_scores).await {
            Ok(data) => data,
            Err(why) => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;
                bail!("Error while creating embed: {}", why);
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
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
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
    recent_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's most recent mania play")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rm")]
pub async fn recentmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's most recent taiko play")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rt")]
pub async fn recenttaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's most recent ctb play")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rc")]
pub async fn recentctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_main(GameMode::CTB, ctx, msg, args).await
}
