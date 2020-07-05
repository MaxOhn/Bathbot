use crate::{
    arguments::{ModSelection, NameModArgs},
    database::MySQL,
    embeds::{EmbedData, LeaderboardEmbed},
    pagination::{LeaderboardPagination, Pagination},
    scraper::Scraper,
    util::{
        globals::{AVATAR_URL, OSU_API_ISSUE},
        MessageExt,
    },
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::RecentRequest,
    models::{
        ApprovalStatus::{Approved, Loved, Ranked},
        GameMode, GameMods,
    },
};
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::Message,
    prelude::Context,
};
use std::sync::Arc;

#[allow(clippy::cognitive_complexity)]
async fn recent_lb_send(
    mode: GameMode,
    national: bool,
    ctx: &Context,
    msg: &Message,
    args: Args,
) -> CommandResult {
    let author_name = {
        let data = ctx.data.read().await;
        let links = data.get::<DiscordLinks>().unwrap();
        links.get(msg.author.id.as_u64()).cloned()
    };
    let args = NameModArgs::new(args);
    let (mods, selection) = args
        .mods
        .unwrap_or_else(|| (GameMods::default(), ModSelection::None));
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
    let score = {
        let request = RecentRequest::with_username(&name).mode(mode).limit(1);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        match request.queue(osu).await {
            Ok(mut score) => {
                if let Some(score) = score.pop() {
                    score
                } else {
                    msg.channel_id
                        .say(ctx, format!("No recent plays found for user `{}`", name))
                        .await?
                        .reaction_delete(ctx, msg.author.id)
                        .await;
                    return Ok(());
                }
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
    };
    let map_id = score.beatmap_id.unwrap();

    // Retrieving the score's beatmap
    let (map_to_db, map) = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        match mysql.get_beatmap(map_id).await {
            Ok(map) => (false, map),
            Err(_) => {
                let osu = data.get::<Osu>().unwrap();
                let map = match score.get_beatmap(osu).await {
                    Ok(m) => m,
                    Err(why) => {
                        msg.channel_id
                            .say(ctx, OSU_API_ISSUE)
                            .await?
                            .reaction_delete(ctx, msg.author.id)
                            .await;
                        return Err(why.to_string().into());
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
                msg.channel_id
                    .say(ctx, OSU_API_ISSUE)
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(why.to_string().into());
            }
        }
    };
    let amount = scores.len();

    // Accumulate all necessary data
    let map_copy = if map_to_db { Some(map.clone()) } else { None };
    let first_place_icon = scores
        .first()
        .map(|s| format!("{}{}", AVATAR_URL, s.user_id));
    let data = match LeaderboardEmbed::new(
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
                    ctx,
                    "Some issue while calculating leaderboard data, blame bade",
                )
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Err(why.to_string().into());
        }
    };

    // Sending the embed
    let resp = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            let content = format!(
                "I found {} scores with the specified mods on the map's leaderboard",
                amount
            );
            m.content(content).embed(|e| data.build(e))
        })
        .await;

    // Add map to database if its not in already
    if let Some(map) = map_copy {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        if let Err(why) = mysql.insert_beatmap(&map).await {
            warn!("Could not add map of recent command to DB: {}", why);
        }
    }

    // Skip pagination if too few entries
    if scores.len() <= 10 {
        resp?.reaction_delete(ctx, msg.author.id).await;
        return Ok(());
    }

    // Pagination
    let pagination = LeaderboardPagination::new(
        ctx,
        resp?,
        msg.author.id,
        map,
        scores,
        author_name,
        first_place_icon,
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
#[description = "Display the belgian leaderboard of a map \
                 that a user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rlb")]
pub async fn recentleaderboard(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::STD, true, ctx, msg, args).await
}

#[command]
#[description = "Display the belgian leaderboard of a map \
                 that a mania user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rmlb")]
pub async fn recentmanialeaderboard(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::MNA, true, ctx, msg, args).await
}

#[command]
#[description = "Display the belgian leaderboard of a map \
                 that a taiko user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rtlb")]
pub async fn recenttaikoleaderboard(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::TKO, true, ctx, msg, args).await
}

#[command]
#[description = "Display the belgian leaderboard of a map \
                 that a ctb user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rclb")]
pub async fn recentctbleaderboard(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::CTB, true, ctx, msg, args).await
}

#[command]
#[description = "Display the global leaderboard of a map \
                 that a user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rglb")]
pub async fn recentgloballeaderboard(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::STD, false, ctx, msg, args).await
}

#[command]
#[description = "Display the global leaderboard of a map \
                 that a mania user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rmglb")]
pub async fn recentmaniagloballeaderboard(
    ctx: &Context,
    msg: &Message,
    args: Args,
) -> CommandResult {
    recent_lb_send(GameMode::MNA, false, ctx, msg, args).await
}

#[command]
#[description = "Display the global leaderboard of a map \
                 that a taiko user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rtglb")]
pub async fn recenttaikogloballeaderboard(
    ctx: &Context,
    msg: &Message,
    args: Args,
) -> CommandResult {
    recent_lb_send(GameMode::TKO, false, ctx, msg, args).await
}

#[command]
#[description = "Display the global leaderboard of a map \
                 that a ctb user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rcglb")]
pub async fn recentctbgloballeaderboard(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::CTB, false, ctx, msg, args).await
}
