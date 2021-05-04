use super::{prepare_score, request_user};
use crate::{
    arguments::{Args, NameMapModArgs},
    embeds::{CompareEmbed, EmbedData, NoScoresEmbed},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        osu::{cached_message_extract, map_id_from_history, MapIdType, ModSelection},
        MessageExt,
    },
    BotResult, Context, Name,
};

use rosu_v2::prelude::{GameMods, OsuError, RankStatus::Ranked};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use twilight_model::channel::Message;

#[command]
#[short_desc("Compare a player's score on a map")]
#[long_desc(
    "Display a user's top score on a given map. \n\
     If no map is given, I will choose the last map \
     I can find in the embeds of this channel.\n\
     Mods can be specified."
)]
#[usage("[username] [map url / map id] [+mods]")]
#[example(
    "badewanne3",
    "badewanne3 2240404 +hdhr",
    "badewanne3 https://osu.ppy.sh/beatmapsets/902425#osu/2240404"
)]
#[aliases("c")]
async fn compare(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = NameMapModArgs::new(&ctx, args);

    let map_id_opt = args.map_id.or_else(|| {
        ctx.cache
            .message_extract(msg.channel_id, cached_message_extract)
    });

    let map_id = if let Some(id) = map_id_opt {
        id
    } else {
        let msgs = match ctx.retrieve_channel_history(msg.channel_id).await {
            Ok(msgs) => msgs,
            Err(why) => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                return Err(why.into());
            }
        };

        match map_id_from_history(msgs) {
            Some(id) => id,
            None => {
                let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";

                return msg.error(&ctx, content).await;
            }
        }
    };

    let map_id = match map_id {
        MapIdType::Map(id) => id,
        MapIdType::Set(_) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return msg.error(&ctx, content).await;
        }
    };

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    let arg_mods = match args.mods {
        None | Some(ModSelection::Exclude(_)) => None,
        Some(ModSelection::Exact(mods)) | Some(ModSelection::Include(mods)) => Some(mods),
    };

    let score_fut = ctx.osu().beatmap_user_score(map_id, name.as_str());

    let score_result = match arg_mods {
        None => score_fut.await,
        Some(mods) => score_fut.mods(mods).await,
    };

    // Retrieve user's score on the map
    let mut score = match score_result {
        Ok(mut score) => match prepare_score(&ctx, &mut score.score).await {
            Ok(_) => score,
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        },
        Err(OsuError::NotFound) => return no_scores(ctx, msg, name, map_id, arg_mods).await,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let map = score.score.map.as_ref().unwrap();
    let mapset_id = map.mapset_id;

    // First try to just get the mapset from the DB
    let mapset_fut = ctx.psql().get_beatmapset(mapset_id);
    let user_fut = ctx.osu().user(score.score.user_id).mode(score.score.mode);

    let scores_fut = async {
        if map.status == Ranked {
            let fut = ctx
                .osu()
                .user_scores(score.score.user_id)
                .best()
                .mode(score.score.mode);

            Some(fut.await)
        } else {
            None
        }
    };

    let (user, scores_opt) = match tokio::join!(mapset_fut, user_fut, scores_fut) {
        (_, Err(why), _) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
        (Ok(mapset), Ok(user), scores_opt) => {
            score.score.mapset.replace(mapset);

            (user, scores_opt)
        }
        (Err(_), Ok(user), scores_opt) => {
            let mapset = match ctx.osu().beatmapset(mapset_id).await {
                Ok(mapset) => mapset,
                Err(why) => {
                    let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                    return Err(why.into());
                }
            };

            score.score.mapset.replace(mapset.into());

            (user, scores_opt)
        }
    };

    let mut best = match scores_opt {
        Some(Ok(scores)) => Some(scores),
        None => None,
        Some(Err(why)) => {
            unwind_error!(warn, why, "Failed to get top scores for compare: {}");

            None
        }
    };

    // Accumulate all necessary data
    let mode = score.score.mode;

    let data = match CompareEmbed::new(&user, best.as_deref(), score, arg_mods.is_some()).await {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Sending the embed
    let embed = data.as_builder().build();
    let response = msg.respond_embed(&ctx, embed).await?;

    response.reaction_delete(&ctx, msg.author.id);
    ctx.store_msg(response.id);

    // Process user and their top scores for tracking
    if let Some(ref mut scores) = best {
        if let Err(why) = ctx.psql().store_scores_maps(scores.iter()).await {
            unwind_error!(warn, why, "Error while storing best maps in DB: {}");
        }

        process_tracking(&ctx, mode, scores, Some(&user)).await;
    }

    // Wait for minimizing
    tokio::spawn(async move {
        sleep(Duration::from_secs(45)).await;

        if !ctx.remove_msg(response.id) {
            return;
        }

        let embed_update = ctx
            .http
            .update_message(response.channel_id, response.id)
            .embed(data.into_builder().build())
            .unwrap();

        if let Err(why) = embed_update.await {
            unwind_error!(warn, why, "Error minimizing compare msg: {}");
        }
    });

    Ok(())
}

async fn no_scores(
    ctx: Arc<Context>,
    msg: &Message,
    name: Name,
    map_id: u32,
    mods: Option<GameMods>,
) -> BotResult<()> {
    let map = match ctx.psql().get_beatmap(map_id, true).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => {
                if let Err(why) = ctx.psql().insert_beatmap(&map).await {
                    unwind_error!(warn, why, "Error while inserting compare map: {}");
                }

                map
            }
            Err(OsuError::NotFound) => {
                let content = format!("There is no map with id {}", map_id);

                return msg.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = msg.send_response(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        },
    };

    let user = match request_user(&ctx, name.as_str(), Some(map.mode)).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{}`", name);

            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Sending the embed
    let embed = NoScoresEmbed::new(user, map, mods).into_builder().build();

    msg.respond_embed(&ctx, embed)
        .await?
        .reaction_delete(&ctx, msg.author.id);

    Ok(())
}
