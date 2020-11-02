use crate::{
    arguments::{Args, NameModArgs},
    bail,
    embeds::{EmbedData, LeaderboardEmbed},
    pagination::{LeaderboardPagination, Pagination},
    util::{
        constants::{AVATAR_URL, GENERAL_ISSUE, OSU_API_ISSUE, OSU_WEB_ISSUE},
        osu::ModSelection,
        MessageExt,
    },
    BotResult, Context,
};

use rosu::{
    backend::requests::RecentRequest,
    models::{
        ApprovalStatus::{Approved, Loved, Ranked},
        GameMode,
    },
};
use std::sync::Arc;
use twilight_model::channel::Message;

#[allow(clippy::cognitive_complexity)]
async fn recent_lb_main(
    mode: GameMode,
    national: bool,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let author_name = ctx.get_link(msg.author.id.0);
    let args = NameModArgs::new(&ctx, args);
    let selection = args.mods;
    let name = match args.name.or_else(|| author_name.clone()) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    // Retrieve the recent scores
    let score_fut = match RecentRequest::with_username(&name) {
        Ok(req) => req.mode(mode).limit(1),
        Err(_) => {
            let content = format!("Could not build request for osu name `{}`", name);
            return msg.error(&ctx, content).await;
        }
    };
    let score = match score_fut.queue(ctx.osu()).await {
        Ok(mut scores) => match scores.pop() {
            Some(score) => score,
            None => {
                let content = format!("No recent plays found for user `{}`", name);
                return msg.error(&ctx, content).await;
            }
        },
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };
    let map_id = score.beatmap_id.unwrap();

    // Retrieving the score's beatmap
    let (map_to_db, map) = {
        match ctx.psql().get_beatmap(map_id).await {
            Ok(map) => (false, map),
            Err(_) => {
                let map = match score.get_beatmap(ctx.osu()).await {
                    Ok(m) => m,
                    Err(why) => {
                        let _ = msg.error(&ctx, OSU_API_ISSUE).await;
                        return Err(why.into());
                    }
                };
                (
                    map.approval_status == Ranked
                        || map.approval_status == Loved
                        || map.approval_status == Approved,
                    map,
                )
            }
        }
    };

    // Retrieve the map's leaderboard
    let scores_fut = ctx.clients.custom.get_leaderboard(
        map_id,
        national,
        match selection {
            Some(ModSelection::Exclude(_)) | None => None,
            Some(ModSelection::Include(m)) | Some(ModSelection::Exact(m)) => Some(m),
        },
        mode,
    );
    let scores = match scores_fut.await {
        Ok(scores) => scores,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_WEB_ISSUE).await;
            return Err(why);
        }
    };
    let amount = scores.len();

    // Accumulate all necessary data
    let map_copy = if map_to_db { Some(map.clone()) } else { None };
    let first_place_icon = scores
        .first()
        .map(|s| format!("{}{}", AVATAR_URL, s.user_id));
    let data_fut = LeaderboardEmbed::new(
        &ctx,
        author_name.as_deref(),
        &map,
        if scores.is_empty() {
            None
        } else {
            Some(scores.iter().take(10))
        },
        &first_place_icon,
        0,
    );
    let data = match data_fut.await {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            bail!("error while creating embed: {}", why);
        }
    };

    // Sending the embed
    let embed = data.build().build()?;
    let content = format!(
        "I found {} scores with the specified mods on the map's leaderboard",
        amount
    );
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .content(content)?
        .embed(embed)?
        .await?;

    // Add map to database if its not in already
    if let Some(map) = map_copy {
        if let Err(why) = ctx.psql().insert_beatmap(&map).await {
            warn!("Could not add map to DB: {}", why);
        }
    }

    // Skip pagination if too few entries
    if scores.len() <= 10 {
        response.reaction_delete(&ctx, msg.author.id);
        return Ok(());
    }

    // Pagination
    let pagination = LeaderboardPagination::new(
        Arc::clone(&ctx),
        response,
        map,
        scores,
        author_name,
        first_place_icon,
    );
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            warn!("Pagination error (recentleaderboard): {}", why)
        }
    });
    Ok(())
}

#[command]
#[short_desc("Belgian leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the belgian leaderboard of a map \
     that a user recently played. Mods can be specified"
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rlb")]
pub async fn recentleaderboard(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_lb_main(GameMode::STD, true, ctx, msg, args).await
}

#[command]
#[short_desc("Belgian leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the belgian leaderboard of a map \
     that a mania user recently played. Mods can be specified"
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rmlb")]
pub async fn recentmanialeaderboard(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_lb_main(GameMode::MNA, true, ctx, msg, args).await
}

#[command]
#[short_desc("Belgian leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the belgian leaderboard of a map \
     that a taiko user recently played. Mods can be specified"
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rtlb")]
pub async fn recenttaikoleaderboard(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_lb_main(GameMode::TKO, true, ctx, msg, args).await
}

#[command]
#[short_desc("Belgian leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the belgian leaderboard of a map \
     that a ctb user recently played. Mods can be specified"
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rclb")]
pub async fn recentctbleaderboard(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_lb_main(GameMode::CTB, true, ctx, msg, args).await
}

#[command]
#[short_desc("Global leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the global leaderboard of a map \
     that a user recently played. Mods can be specified"
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rglb")]
pub async fn recentgloballeaderboard(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
) -> BotResult<()> {
    recent_lb_main(GameMode::STD, false, ctx, msg, args).await
}

#[command]
#[short_desc("Global leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the global leaderboard of a map \
     that a mania user recently played. Mods can be specified"
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rmglb")]
pub async fn recentmaniagloballeaderboard(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
) -> BotResult<()> {
    recent_lb_main(GameMode::MNA, false, ctx, msg, args).await
}

#[command]
#[short_desc("Global leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the global leaderboard of a map \
     that a taiko user recently played. Mods can be specified"
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rtglb")]
pub async fn recenttaikogloballeaderboard(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
) -> BotResult<()> {
    recent_lb_main(GameMode::TKO, false, ctx, msg, args).await
}

#[command]
#[short_desc("Global leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the global leaderboard of a map \
     that a ctb user recently played. Mods can be specified"
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rcglb")]
pub async fn recentctbgloballeaderboard(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
) -> BotResult<()> {
    recent_lb_main(GameMode::CTB, false, ctx, msg, args).await
}
