use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, RecentListEmbed},
    pagination::{Pagination, RecentListPagination},
    unwind_error,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        numbers, MessageExt,
    },
    BotResult, Context,
};

use rosu::model::GameMode;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use twilight_model::channel::Message;

async fn recent_list_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = NameArgs::new(&ctx, args);

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    // Retrieve the user and their recent scores
    let user_fut = ctx.osu().user(name.as_str()).mode(mode);
    let scores_fut = ctx.osu().recent_scores(name.as_str()).mode(mode).limit(50);

    let (user, scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((None, _)) => {
            let content = format!("User `{}` was not found", name);

            return msg.error(&ctx, content).await;
        }
        Ok((_, scores)) if scores.is_empty() => {
            let content = format!(
                "No recent {}plays found for user `{}`",
                match mode {
                    GameMode::STD => "",
                    GameMode::TKO => "taiko ",
                    GameMode::CTB => "ctb ",
                    GameMode::MNA => "mania ",
                },
                name
            );

            return msg.error(&ctx, content).await;
        }
        Ok((Some(user), scores)) => (user, scores),
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
                unwind_error!(warn, why, "Error while retrieving maps from DB: {}");

                HashMap::default()
            }
        }
    };

    // Memoize which maps are already in the DB
    map_ids.retain(|id| maps.contains_key(&id));

    // Prepare the maps
    for score in scores.iter().take(10) {
        let map_id = score.beatmap_id.unwrap();

        // Make sure map is ready
        #[allow(clippy::clippy::map_entry)]
        if !maps.contains_key(&map_id) {
            let map = ctx
                .osu()
                .beatmap()
                .map_id(score.beatmap_id.unwrap())
                .await?
                .unwrap();

            maps.insert(map_id, map);
        }
    }

    let pages = numbers::div_euclid(10, scores.len());
    let scores_iter = scores.iter().take(10);

    let data = match RecentListEmbed::new(&user, &maps, scores_iter, (1, pages)).await {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let embed = data.build_owned().build()?;

    let response = ctx
        .http
        .create_message(msg.channel_id)
        .embed(embed)?
        .await?;

    // Skip pagination if too few entries
    if scores.len() <= 10 {
        response.reaction_delete(&ctx, msg.author.id);

        return Ok(());
    }

    // Pagination
    let pagination =
        RecentListPagination::new(Arc::clone(&ctx), response, user, scores, maps, map_ids);
    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (recentlist): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display a list of a user's most recent plays")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rl")]
pub async fn recentlist(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_list_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Display a list of a user's most recent mania plays")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rlm")]
pub async fn recentlistmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_list_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Display a list of a user's most recent taiko plays")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rlt")]
pub async fn recentlisttaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_list_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Display a list of a user's most recent ctb plays")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rlc")]
pub async fn recentlistctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_list_main(GameMode::CTB, ctx, msg, args).await
}
