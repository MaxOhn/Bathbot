use crate::{
    arguments::{Args, NameMapArgs},
    embeds::{EmbedData, ScoresEmbed},
    pagination::{Pagination, ScoresPagination},
    unwind_error,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        osu::{cached_message_extract, map_id_from_history, MapIdType},
        MessageExt,
    },
    BotResult, Context,
};

use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("Each mod's top score from a player on a map")]
#[long_desc(
    "Display a user's top score for each mod on a given map. \n\
     If no map is given, I will choose the last map \
     I can find in the embeds of this channel"
)]
#[usage("[username] [map url / map id]")]
#[example(
    "badewanne3",
    "badewanne3 2240404",
    "badewanne3 https://osu.ppy.sh/beatmapsets/902425#osu/2240404"
)]
#[aliases("c", "compare")]
async fn scores(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = NameMapArgs::new(&ctx, args);

    let map_id = if let Some(id) = args.map_id {
        match id {
            MapIdType::Map(id) => id,
            MapIdType::Set(_) => {
                let content = "Looks like you gave me a mapset id, I need a map id though";
                return msg.error(&ctx, content).await;
            }
        }
    } else if let Some(id) = ctx
        .cache
        .message_extract(msg.channel_id, cached_message_extract)
    {
        id.id()
    } else {
        let msgs = match ctx.retrieve_channel_history(msg.channel_id).await {
            Ok(msgs) => msgs,
            Err(why) => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                return Err(why.into());
            }
        };

        match map_id_from_history(msgs) {
            Some(MapIdType::Map(id)) => id,
            Some(MapIdType::Set(_)) => {
                let content = "Looks like you gave me a mapset id, I need a map id though";

                return msg.error(&ctx, content).await;
            }
            None => {
                let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";

                return msg.error(&ctx, content).await;
            }
        }
    };

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    // Retrieving the beatmap
    let map = match ctx.psql().get_beatmap(map_id).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(Some(map)) => map,
            Ok(None) => {
                let content = format!(
                    "Could not find beatmap with id `{}`. \
                        Did you give me a mapset id instead of a map id?",
                    map_id
                );

                return msg.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        },
    };

    // Retrieve user and their scores on the map
    let user_fut = ctx.osu().user(name.as_str()).mode(map.mode);
    let scores_fut = ctx.osu().scores(map_id).user(name.as_str()).mode(map.mode);

    let (user, scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((Some(user), scores)) => (user, scores),
        Ok((None, _)) => {
            let content = format!("Could not find user `{}`", name);

            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let init_scores = scores.iter().take(10);

    // Accumulate all necessary data
    let data = ScoresEmbed::new(&user, &map, init_scores, 0).await;

    // Sending the embed
    let embed = data.build().build()?;

    let response = ctx
        .http
        .create_message(msg.channel_id)
        .embed(embed)?
        .await?;

    // Add map to database if its not in already
    if let Err(why) = ctx.clients.psql.insert_beatmap(&map).await {
        unwind_error!(warn, why, "Error while adding new map to DB: {}");
    }

    // Skip pagination if too few entries
    if scores.len() <= 10 {
        response.reaction_delete(&ctx, msg.author.id);

        return Ok(());
    }

    // Pagination
    let pagination = ScoresPagination::new(response, user, map, scores);
    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (scores): {}")
        }
    });

    Ok(())
}
