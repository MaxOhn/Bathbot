use crate::{
    arguments::{Args, NameMapArgs},
    embeds::{EmbedData, ScoresEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        discord, MessageExt,
    },
    BotResult, Context,
};

use rosu::backend::requests::{BeatmapRequest, ScoreRequest, UserRequest};
use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Each mod's top score from a player on a map")]
#[long_desc(
    "Display a user's top score for each mod on a given map. \
     If no map is given, I will choose the last map \
     I can find in my embeds of this channel"
)]
#[usage("[username] [map url / map id]")]
#[example("badewanne3")]
#[example("badewanne3 2240404")]
#[example("badewanne3 https://osu.ppy.sh/beatmapsets/902425#osu/2240404")]
#[aliases("c", "compare")]
async fn scores(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let args = NameMapArgs::new(Args::new(msg.content.clone()));
    let map_id = if let Some(map_id) = args.map_id {
        map_id
    } else {
        let msgs = msg
            .channel_id
            .messages(ctx, |retriever| retriever.limit(50))
            .await?;
        match discord::map_id_from_history(msgs, &ctx.cache).await {
            Some(id) => id,
            None => {
                let content = "No map embed found in this channel's recent history.\n\
                         Try specifying a map as last argument either by url to the map, \
                         or just by map id.";
                msg.respond(&ctx, contenet).await?;
                return Ok(());
            }
        }
    };
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

    // Retrieving the beatmap
    let map = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        match mysql.get_beatmap(map_id).await {
            Ok(map) => map,
            Err(_) => {
                let map_req = BeatmapRequest::new().map_id(map_id);
                let osu = data.get::<Osu>().unwrap();
                match map_req.queue_single(&osu).await {
                    Ok(result) => match result {
                        Some(map) => map,
                        None => {
                            let content = format!(
                                "Could not find beatmap with id `{}`. \
                                Did you give me a mapset id instead of a map id?",
                                map_id
                            );
                            msg.respond(&ctx, content).await?;
                            return Ok(());
                        }
                    },
                    Err(why) => {
                        msg.respond(&ctx, OSU_API_ISSUE).await?;
                        return Err(why.into());
                    }
                }
            }
        }
    };

    // Retrieve user and user's scores on the map
    let (user, map, scores) = {
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        let score_req = ScoreRequest::with_map_id(map_id)
            .username(&name)
            .mode(map.mode);
        let scores = match score_req.queue(osu).await {
            Ok(scores) => scores,
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        };
        let user_req = UserRequest::with_username(&name).mode(map.mode);
        let user = match user_req.queue_single(osu).await {
            Ok(result) => match result {
                Some(user) => user,
                None => {
                    let content = format!("Could not find user `{}`", name);
                    msg.respond(&ctx, content).await?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        };
        (user, map, scores)
    };

    // Accumulate all necessary data
    let data = match ScoresEmbed::new(user, &map, scores, ctx).await {
        Ok(data) => data,
        Err(why) => {
            msg.respond(&ctx, GENERAL_ISSUE).await?;
            return Err(why);
        }
    };

    // Sending the embed
    let response = msg
        .channel_id
        .send_message(ctx, |m| m.embed(|e| data.build(e)))
        .await;

    // Add map to database if its not in already
    {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        if let Err(why) = mysql.insert_beatmap(&map).await {
            warn!("Could not add map of compare command to DB: {}", why);
        }
    }
    response?.reaction_delete(ctx, msg.author.id).await;
    Ok(())
}
